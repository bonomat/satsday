use crate::config::Config;
use crate::esplora::EsploraClient;
use crate::games::GameType;
use crate::key_derivation::KeyDerivation;
use crate::key_derivation::Multiplier;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use ark_core::{batch, intent};
use ark_core::batch::{create_and_sign_forfeit_txs, sign_batch_tree_tx, aggregate_nonces};
use ark_core::batch::generate_nonce_tree;
use ark_core::batch::sign_commitment_psbt;
use ark_core::boarding_output::list_boarding_outpoints;
use ark_core::boarding_output::BoardingOutpoints;
use ark_core::coin_select::select_vtxos;
use ark_core::send;
use ark_core::send::build_offchain_transactions;
use ark_core::send::sign_ark_transaction;
use ark_core::send::sign_checkpoint_transaction;
use ark_core::send::OffchainTransactions;
use ark_core::server::{BatchTreeEventType, PartialSigTree, SubscriptionResponse};
use ark_core::server::GetVtxosRequest;
use ark_core::server::StreamEvent;
use ark_core::vtxo::list_virtual_tx_outpoints;
use ark_core::vtxo::VirtualTxOutPoints;
use ark_core::ArkAddress;
use ark_core::BoardingOutput;
use ark_core::TxGraph;
use ark_core::Vtxo;
use bitcoin::hashes::sha256;
use bitcoin::hashes::Hash;
use bitcoin::hex::DisplayHex;
use bitcoin::key::Keypair;
use bitcoin::key::Secp256k1;
use bitcoin::key::TweakedPublicKey;
use bitcoin::secp256k1::schnorr;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::{self};
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::TxOut;
use bitcoin::Txid;
use bitcoin::XOnlyPublicKey;
use futures::StreamExt;
use rand::thread_rng;
use std::collections::HashMap;
use tokio::task::block_in_place;

pub struct ArkClient {
    grpc_client: ark_grpc::Client,
    esplora_client: EsploraClient,
    server_info: ark_core::server::Info,
    main_address: (Vtxo, SecretKey),
    boarding_output: BoardingOutput,
    secp: Secp256k1<secp256k1::All>,
    game_addresses: Vec<GameArkAddress>,
}

#[derive(Debug)]
pub struct Balance {
    pub offchain_spendable: Amount,
    pub offchain_expired: Amount,
    pub boarding_spendable: Amount,
    pub boarding_expired: Amount,
    pub boarding_pending: Amount,
}

#[derive(Debug, Clone)]
pub struct SubscriptionEvent {
    pub txid: Txid,
    pub vout: u32,
    pub amount: Amount,
    pub script_pubkey: bitcoin::ScriptBuf,
}

impl ArkClient {
    pub async fn new(config: Config) -> Result<Self> {
        let secp = Secp256k1::new();

        // Read master seed and create key derivation
        let master_seed = std::fs::read_to_string(&config.master_seed_file)
            .with_context(|| {
                format!(
                    "Failed to read master seed file: {}",
                    config.master_seed_file
                )
            })?
            .trim()
            .to_string();

        let key_derivation = KeyDerivation::from_seed(&master_seed, bitcoin::Network::Bitcoin)?;

        // Derive main key
        let main_sk_bytes = key_derivation.get_main_secret_key()?;
        let main_sk = SecretKey::from_slice(&main_sk_bytes)?;
        let main_pk = PublicKey::from_secret_key(&secp, &main_sk);

        let mut grpc_client = ark_grpc::Client::new(config.ark_server_url);
        grpc_client.connect().await?;

        let server_info = grpc_client.get_info().await?;
        let esplora_client = EsploraClient::new(&config.esplora_url)?;

        // Create main VTXO
        let main_vtxo = Vtxo::new_default(
            &secp,
            server_info.signer_pk.x_only_public_key().0,
            main_pk.x_only_public_key().0,
            server_info.unilateral_exit_delay,
            server_info.network,
        )?;


        // Create boarding output
        let boarding_output = BoardingOutput::new(
            &secp,
            server_info.signer_pk.x_only_public_key().0,
            main_pk.x_only_public_key().0,
            server_info.boarding_exit_delay,
            server_info.network,
        )?;

        // Generate all game addresses using key derivation
        let mut game_addresses = Vec::new();
        for multiplier in Multiplier::all() {
            let game_sk_bytes = key_derivation.get_game_secret_key(multiplier)?;
            let game_sk = SecretKey::from_slice(&game_sk_bytes)?;
            let game_pk = PublicKey::from_secret_key(&secp, &game_sk);

            let game_vtxo = Vtxo::new_default(
                &secp,
                server_info.signer_pk.x_only_public_key().0,
                game_pk.x_only_public_key().0,
                server_info.unilateral_exit_delay,
                server_info.network,
            )?;

            game_addresses.push(GameArkAddress {
                game_type: GameType::SatoshisNumber, // For now, all addresses are SatoshisNumber
                multiplier,
                vtxo: game_vtxo,
                secret_key: game_sk,
            });
        }

        Ok(Self {
            grpc_client,
            esplora_client,
            server_info,
            main_address: (main_vtxo, main_sk),
            game_addresses,
            boarding_output,
            secp,
        })
    }

    pub fn get_address(&self) -> ArkAddress {
        self.main_address.0.to_ark_address()
    }

    pub fn get_boarding_address(&self) -> bitcoin::Address {
        self.boarding_output.address().clone()
    }

    pub async fn get_balance(&self) -> Result<Balance> {
        let runtime = tokio::runtime::Handle::current();
        let find_outpoints_fn =
            |address: &bitcoin::Address| -> Result<Vec<ark_core::ExplorerUtxo>, ark_core::Error> {
                block_in_place(|| {
                    runtime.block_on(async {
                        let outpoints = self
                            .esplora_client
                            .find_outpoints(address)
                            .await
                            .map_err(ark_core::Error::ad_hoc)?;
                        Ok(outpoints)
                    })
                })
            };

        let virtual_tx_outpoints = {
            let spendable_vtxos = self.spendable_vtxos(false).await?;
            list_virtual_tx_outpoints(find_outpoints_fn, spendable_vtxos)?
        };

        let boarding_outpoints =
            list_boarding_outpoints(find_outpoints_fn, &[self.boarding_output.clone()])?;

        Ok(Balance {
            offchain_spendable: virtual_tx_outpoints.spendable_balance(),
            offchain_expired: virtual_tx_outpoints.expired_balance(),
            boarding_spendable: boarding_outpoints.spendable_balance(),
            boarding_expired: boarding_outpoints.expired_balance(),
            boarding_pending: boarding_outpoints.pending_balance(),
        })
    }

    pub async fn send_offchain(&self, address_amounts: Vec<(&ArkAddress, Amount)>) -> Result<Txid> {
        for (address, amount) in &address_amounts {
            tracing::debug!(target: "client", address = address.encode(), amount = amount.to_string(), "Sending money to");
        }

        // Recoverable VTXOs cannot be sent.
        let select_recoverable_vtxos = false;

        let spendable_vtxos = self.spendable_offchain_vtxos(select_recoverable_vtxos).await?;

        // Run coin selection algorithm on candidate spendable VTXOs.
        let spendable_virtual_tx_outpoints = spendable_vtxos
            .iter()
            .flat_map(|(_, vtxos)| vtxos.clone())
            .map(|vtxo| ark_core::coin_select::VirtualTxOutPoint {
                outpoint: vtxo.outpoint,
                expire_at: vtxo.expires_at,
                amount: vtxo.amount,
            })
            .collect::<Vec<_>>();

        let total_amount = address_amounts.iter().map(|(_, amount)| *amount).sum();

        let selected_coins = if total_amount == Amount::ZERO {
            spendable_virtual_tx_outpoints
        } else {
            select_vtxos(
                spendable_virtual_tx_outpoints,
                total_amount,
                self.server_info.dust,
                true,
            )?
        };

        let vtxo_inputs = selected_coins
            .into_iter()
            .map(|virtual_tx_outpoint| {
                let vtxo = spendable_vtxos
                    .clone()
                    .into_iter()
                    .find_map(|(vtxo, virtual_tx_outpoints)| {
                        virtual_tx_outpoints
                            .iter()
                            .any(|v| v.outpoint == virtual_tx_outpoint.outpoint)
                            .then_some(vtxo)
                    })
                    .expect("to find matching VTXO");

                let (forfeit_script, control_block) = vtxo
                    .forfeit_spend_info()
                    .context("failed to get forfeit spend info")?;

                Ok(send::VtxoInput::new(
                    forfeit_script,
                    None,
                    control_block,
                    vtxo.tapscripts(),
                    vtxo.script_pubkey(),
                    virtual_tx_outpoint.amount,
                    virtual_tx_outpoint.outpoint,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let (main_address, _) = &self.main_address;
        let change_address = main_address.to_ark_address();

        let OffchainTransactions {
            mut ark_tx,
            checkpoint_txs,
        } = build_offchain_transactions(
            address_amounts.as_slice(),
            Some(&change_address),
            &vtxo_inputs,
            &self.server_info,
        )?;

        let mut all_keys = vec![self.main_address.clone()];
        for game_address in &self.game_addresses {
            all_keys.push((game_address.vtxo.clone(), game_address.secret_key));
        }

        let sign_fn = |_: &mut bitcoin::psbt::Input,
                       msg: secp256k1::Message|
         -> Result<(schnorr::Signature, XOnlyPublicKey), ark_core::Error> {
            let kp = all_keys.iter().find_map(|(v, sk)| {
                // Try to match against all VTXOs we're spending
                vtxo_inputs.iter().find_map(|input| {
                    if input.spend_info().0 == &v.script_pubkey() {
                        Some(sk.keypair(&self.secp))
                    } else {
                        None
                    }
                })
            });
            let kp = kp
                .context("Key not found for vtxo")
                .map_err(ark_core::Error::ad_hoc)?;

            let sig = self.secp.sign_schnorr_no_aux_rand(&msg, &kp);
            let pk = kp.x_only_public_key().0;
            Ok((sig, pk))
        };

        for i in 0..checkpoint_txs.len() {
            sign_ark_transaction(sign_fn, &mut ark_tx, i)?;
        }

        let ark_txid = ark_tx.unsigned_tx.compute_txid();

        let mut res = self
            .grpc_client
            .submit_offchain_transaction_request(ark_tx, checkpoint_txs)
            .await
            .context("failed to submit offchain transaction request")?;

        for checkpoint_psbt in res.signed_checkpoint_txs.iter_mut() {
            sign_checkpoint_transaction(sign_fn, checkpoint_psbt)?;
        }

        self.grpc_client
            .finalize_offchain_transaction(ark_txid, res.signed_checkpoint_txs)
            .await
            .context("failed to finalize offchain transaction")?;

        Ok(ark_txid)
    }

    pub async fn settle(&self) -> Result<Option<Txid>> {
        let runtime = tokio::runtime::Handle::current();
        let find_outpoints_fn =
            |address: &bitcoin::Address| -> Result<Vec<ark_core::ExplorerUtxo>, ark_core::Error> {
                block_in_place(|| {
                    runtime.block_on(async {
                        let outpoints = self
                            .esplora_client
                            .find_outpoints(address)
                            .await
                            .map_err(ark_core::Error::ad_hoc)?;
                        Ok(outpoints)
                    })
                })
            };

        let virtual_tx_outpoints = {
            let spendable_vtxos = self.spendable_vtxos(true).await?;
            list_virtual_tx_outpoints(find_outpoints_fn, spendable_vtxos)?
        };

        let boarding_outpoints =
            list_boarding_outpoints(find_outpoints_fn, &[self.boarding_output.clone()])?;

        self.settle_internal(virtual_tx_outpoints, boarding_outpoints)
            .await
    }

    pub async fn spendable_vtxos(
        &self,
        select_recoverable_vtxos: bool,
    ) -> Result<HashMap<Vtxo, Vec<ark_core::server::VirtualTxOutPoint>>> {
        let mut spendable_vtxos = HashMap::new();

        let main = self
            ._spendable_vtxos(self.main_address.0.clone(), select_recoverable_vtxos)
            .await?;
        spendable_vtxos.insert(main.0, main.1);
        for game_address in &self.game_addresses {
            let spendable = self
                ._spendable_vtxos(game_address.vtxo.clone(), select_recoverable_vtxos)
                .await?;
            spendable_vtxos.insert(spendable.0, spendable.1);
        }

        Ok(spendable_vtxos)
    }

    pub async fn spendable_offchain_vtxos(
        &self,
        select_recoverable_vtxos: bool,
    ) -> Result<HashMap<Vtxo, Vec<ark_core::server::VirtualTxOutPoint>>> {
        let main = self
            ._spendable_vtxos(self.main_address.0.clone(), select_recoverable_vtxos)
            .await?;

        let game_addressess = self
            .game_addresses
            .iter()
            .map(|a| a.vtxo.to_ark_address())
            .collect::<Vec<_>>();

        let request = GetVtxosRequest::new_for_addresses(game_addressess.as_slice());

        let vtxo_outpoints = self.grpc_client.list_vtxos(request).await?;

        let spendable = if select_recoverable_vtxos {
            vtxo_outpoints.spendable_with_recoverable()
        } else {
            vtxo_outpoints.spendable().to_vec()
        };

        let mut spendable_vtxos = HashMap::new();
        spendable_vtxos.insert(main.0, main.1);

        for game_address in &self.game_addresses {
            let outpoints = spendable
                .clone()
                .into_iter()
                .filter(|vtop| vtop.script == game_address.vtxo.script_pubkey())
                .collect::<Vec<_>>();
            spendable_vtxos.insert(game_address.vtxo.clone(), outpoints);
        }

        Ok(spendable_vtxos)
    }

    async fn _spendable_vtxos(
        &self,
        vtxo: Vtxo,
        select_recoverable_vtxos: bool,
    ) -> Result<(Vtxo, Vec<ark_core::server::VirtualTxOutPoint>)> {
        let request = GetVtxosRequest::new_for_addresses(&[vtxo.to_ark_address()]);

        let vtxo_outpoints = self.grpc_client.list_vtxos(request).await?;

        let spendable = if select_recoverable_vtxos {
            vtxo_outpoints.spendable_with_recoverable()
        } else {
            vtxo_outpoints.spendable().to_vec()
        };

        Ok((vtxo, spendable))
    }

    /// Lists all VTXOs for the given addresses, spent, recoverable and unspent
    pub async fn list_vtxos(
        &self,
        addresses: &[ArkAddress],
    ) -> Result<Vec<ark_core::server::VirtualTxOutPoint>> {
        let request = GetVtxosRequest::new_for_addresses(addresses);

        let vtxo_outpoints = self.grpc_client.list_vtxos(request).await?;

        Ok(vtxo_outpoints.all())
    }

    async fn settle_internal(
        &self,
        vtxos: VirtualTxOutPoints,
        boarding_outputs: BoardingOutpoints,
    ) -> Result<Option<Txid>> {
        let mut rng = thread_rng();

        if vtxos.spendable.is_empty() && boarding_outputs.spendable.is_empty() {
            return Ok(None);
        }

        let cosigner_kp = Keypair::new(&self.secp, &mut rng);
        let (main_address, main_sk) = &self.main_address;
        let to_address = main_address.to_ark_address();

        let round_inputs = {
            let boarding_inputs = boarding_outputs.spendable.clone().into_iter().map(
                |(outpoint, amount, boarding_output)| {
                    intent::Input::new(
                        outpoint,
                        boarding_output.exit_delay(),
                        TxOut {
                            value: amount,
                            script_pubkey: boarding_output.script_pubkey(),
                        },
                        boarding_output.tapscripts(),
                        boarding_output.owner_pk(),
                        boarding_output.forfeit_spend_info(),
                        true,
                    )
                },
            );

            let vtxo_inputs = vtxos
                .spendable
                .clone()
                .into_iter()
                .map(|(virtual_tx_outpoint, vtxo)| {
                    Ok(intent::Input::new(
                        virtual_tx_outpoint.outpoint,
                        vtxo.exit_delay(),
                        TxOut {
                            value: virtual_tx_outpoint.amount,
                            script_pubkey: vtxo.script_pubkey(),
                        },
                        vtxo.tapscripts(),
                        vtxo.owner_pk(),
                        vtxo.forfeit_spend_info()
                            .context("failed to get forfeit spend info")?,
                        false,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;

            boarding_inputs.chain(vtxo_inputs).collect::<Vec<_>>()
        };
        let n_round_inputs = round_inputs.len();

        let spendable_amount = boarding_outputs.spendable_balance() + vtxos.spendable_balance();
        let round_outputs = vec![intent::Output::Offchain(TxOut {
            value: spendable_amount,
            script_pubkey: to_address.to_p2tr_script_pubkey(),
        })];

        let own_cosigner_kps = [cosigner_kp];
        let own_cosigner_pks = own_cosigner_kps
            .iter()
            .map(|k| k.public_key())
            .collect::<Vec<_>>();

        let main_signing_kp = Keypair::from_secret_key(&self.secp, main_sk);
        let mut signing_kps = self
            .game_addresses
            .iter()
            .map(|game_ark_address| game_ark_address.secret_key.keypair(&self.secp))
            .collect::<Vec<_>>();
        signing_kps.push(main_signing_kp);

        let sign_for_onchain_pk_fn = |xonly_public_key: &XOnlyPublicKey,
                                      msg: &secp256k1::Message|
         -> Result<schnorr::Signature, ark_core::Error> {
            tracing::debug!("Signing for key {xonly_public_key}");
            Ok(self.secp.sign_schnorr_no_aux_rand(msg, &main_signing_kp))
        };

        let signature = intent::make_intent(
            signing_kps.as_slice(),
            sign_for_onchain_pk_fn,
            round_inputs,
            round_outputs,
            own_cosigner_pks.clone(),
        )?;

        let intent_id = self
            .grpc_client
            .register_intent(signature)
            .await?;

        let topics = vtxos
            .spendable
            .iter()
            .map(|(o, _)| o.outpoint.to_string())
            .chain(
                own_cosigner_pks
                    .iter()
                    .map(|pk| pk.serialize().to_lower_hex_string()),
            )
            .collect();

        let mut event_stream = self.grpc_client.get_event_stream(topics).await?;

        let mut vtxo_graph_chunks = Vec::new();

        let batch_started_event = match event_stream.next().await {
            Some(Ok(StreamEvent::BatchStarted(e))) => e,
            other => bail!("Did not get round signing event: {other:?}"),
        };

        let hash = sha256::Hash::hash(intent_id.as_bytes());
        let hash = hash.as_byte_array().to_vec().to_lower_hex_string();

        if batch_started_event
            .intent_id_hashes
            .iter()
            .any(|h| h == &hash)
        {
            self.grpc_client
                .confirm_registration(intent_id.clone())
                .await?;
        } else {
            bail!(
                "Did not find intent ID {} in round: {}",
                intent_id,
                batch_started_event.id
            )
        }

        let round_signing_event;
        loop {
            match event_stream.next().await {
                Some(Ok(StreamEvent::TreeTx(e))) => match e.batch_tree_event_type {
                    BatchTreeEventType::Vtxo => vtxo_graph_chunks.push(e.tx_graph_chunk),
                    BatchTreeEventType::Connector => {
                        bail!("Unexpected connector batch tree event");
                    }
                },
                Some(Ok(StreamEvent::TreeSigningStarted(e))) => {
                    round_signing_event = e;
                    break;
                }
                other => bail!("Unexpected event while waiting for round signing: {other:?}"),
            }
        }

        let mut vtxo_graph = TxGraph::new(vtxo_graph_chunks)?;

        let round_id = round_signing_event.id;

        let nonce_tree = generate_nonce_tree(
            &mut rng,
            &vtxo_graph,
            cosigner_kp.public_key(),
            &round_signing_event.unsigned_commitment_tx,
        )?;

        self.grpc_client
            .submit_tree_nonces(
                &round_id,
                cosigner_kp.public_key(),
                nonce_tree.to_nonce_pks(),
            )
            .await?;

        let mut agg_nonce_pks = HashMap::new();
        let batch_expiry = batch_started_event.batch_expiry;
        let (ark_forfeit_pk, _) = self.server_info.forfeit_pk.x_only_public_key();
        let mut connectors_graph_chunks = Vec::new();
        let mut nonce_tree = Some(nonce_tree);

        let round_finalization_event;
        loop {
            match event_stream.next().await {
                Some(Ok(StreamEvent::TreeTx(e))) => match e.batch_tree_event_type {
                    BatchTreeEventType::Vtxo => {
                        bail!("Unexpected VTXO batch tree event");
                    }
                    BatchTreeEventType::Connector => {
                        connectors_graph_chunks.push(e.tx_graph_chunk);
                    }
                },
                Some(Ok(StreamEvent::TreeNonces(e))) => {
                    let tree_tx_nonce_pks = e.nonces;

                    // Check if this event is for us
                    let cosigner_pk = match tree_tx_nonce_pks.0.iter().find(|(pk, _)| {
                        &&cosigner_kp.public_key().x_only_public_key().0 == pk
                    }) {
                        Some((pk, _)) => *pk,
                        None => {
                            tracing::debug!(
                                batch_id = e.id,
                                txid = %e.txid,
                                "Received irrelevant TreeNonces event"
                            );
                            continue;
                        }
                    };

                    tracing::debug!(
                        batch_id = e.id,
                        txid = %e.txid,
                        %cosigner_pk,
                        "Received TreeNonces event"
                    );

                    let agg_nonce_pk = aggregate_nonces(tree_tx_nonce_pks);
                    agg_nonce_pks.insert(e.txid, agg_nonce_pk);

                    // Once we have nonces for all transactions, sign the tree
                    if agg_nonce_pks.len() == vtxo_graph.nb_of_nodes() {
                        let mut our_nonce_tree = nonce_tree.take().ok_or_else(|| {
                            anyhow::anyhow!("missing nonce tree during batch protocol")
                        })?;

                        let mut partial_sig_tree = PartialSigTree::default();
                        for (txid, _) in vtxo_graph.as_map() {
                            let agg_nonce_pk = agg_nonce_pks.get(&txid).ok_or_else(|| {
                                anyhow::anyhow!(format!("missing aggregated nonce PK for TX {txid}"))
                            })?;

                            let sigs = sign_batch_tree_tx(
                                txid,
                                batch_expiry,
                                ark_forfeit_pk,
                                &cosigner_kp,
                                *agg_nonce_pk,
                                &vtxo_graph,
                                &round_signing_event.unsigned_commitment_tx,
                                &mut our_nonce_tree,
                            )?;

                            partial_sig_tree.0.extend(sigs.0);
                        }

                        self.grpc_client
                            .submit_tree_signatures(&round_id, cosigner_kp.public_key(), partial_sig_tree)
                            .await?;
                    }
                }
                Some(Ok(StreamEvent::TreeNoncesAggregated(_))) => {
                    // Ignore this event - we handle TreeNonces instead
                }
                Some(Ok(StreamEvent::TreeSignature(e))) => match e.batch_tree_event_type {
                    BatchTreeEventType::Vtxo => {
                        vtxo_graph.apply(|graph| {
                            if graph.root().unsigned_tx.compute_txid() != e.txid {
                                Ok(true)
                            } else {
                                graph.set_signature(e.signature);
                                Ok(false)
                            }
                        })?;
                    }
                    BatchTreeEventType::Connector => {
                        bail!("received batch tree signature for connectors tree");
                    }
                },
                Some(Ok(StreamEvent::BatchFinalization(e))) => {
                    round_finalization_event = e;
                    break;
                }
                other => bail!("Unexpected event while waiting for round finalization: {other:?}"),
            }
        }

        let _round_id = round_finalization_event.id;

        let vtxo_inputs = vtxos
            .spendable
            .into_iter()
            .map(|(outpoint, vtxo)| {
                batch::VtxoInput::new(
                    vtxo,
                    outpoint.amount,
                    outpoint.outpoint,
                    outpoint.is_recoverable(),
                )
            })
            .collect::<Vec<_>>();

        let signed_forfeit_psbts = if !vtxo_inputs.is_empty() {
            let connectors_graph = TxGraph::new(connectors_graph_chunks)?;

            create_and_sign_forfeit_txs(
                vtxo_inputs.as_slice(),
                &connectors_graph.leaves(),
                &self.server_info.forfeit_address,
                self.server_info.dust,
                |msg, vtxo| {
                    let ark_address = vtxo.to_ark_address().encode();
                    let kp = if ark_address == main_address.to_ark_address().encode() {
                        main_signing_kp
                    } else {
                        let maybe_kp = self.game_addresses.iter().find_map(|game_address| {
                            if game_address.vtxo.to_ark_address().encode() == ark_address {
                                Some(game_address.secret_key.keypair(&self.secp))
                            } else {
                                None
                            }
                        });
                        maybe_kp.expect("to have a key")
                    };
                    let sig = self.secp.sign_schnorr_no_aux_rand(msg, &kp);
                    let pk = kp.x_only_public_key().0;
                    (sig, pk)
                },
            )?
        } else {
            Vec::new()
        };

        let onchain_inputs = boarding_outputs
            .spendable
            .into_iter()
            .map(|(outpoint, amount, boarding_output)| {
                batch::OnChainInput::new(boarding_output, amount, outpoint)
            })
            .collect::<Vec<_>>();

        let round_psbt = if n_round_inputs == 0 {
            None
        } else {
            let mut round_psbt = round_finalization_event.commitment_tx;

            let sign_for_pk_fn = |_: &XOnlyPublicKey,
                                  msg: &secp256k1::Message|
             -> Result<schnorr::Signature, ark_core::Error> {
                Ok(self.secp.sign_schnorr_no_aux_rand(msg, &main_signing_kp))
            };

            sign_commitment_psbt(sign_for_pk_fn, &mut round_psbt, &onchain_inputs)?;

            Some(round_psbt)
        };

        self.grpc_client
            .submit_signed_forfeit_txs(signed_forfeit_psbts, round_psbt)
            .await?;

        let round_finalized_event = match event_stream.next().await {
            Some(Ok(StreamEvent::BatchFinalized(e))) => e,
            other => bail!("Did not get round finalized event: {other:?}"),
        };

        Ok(Some(round_finalized_event.commitment_txid))
    }

    pub async fn get_parent_vtxo(&self, out_point: OutPoint) -> Result<Vec<ArkAddress>> {
        tracing::trace!(
            txid = ?out_point.txid,
            "Getting parent vtxo");
        let vtxo = self
            .grpc_client
            .get_virtual_txs(vec![out_point.txid.to_string()], None)
            .await?;
        let parent_checkoints = vtxo
            .txs
            .iter()
            .flat_map(|tx| {
                tx.unsigned_tx
                    .input
                    .iter()
                    .map(|input| input.previous_output)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        if parent_checkoints.is_empty() {
            tracing::warn!("No parent found");
            return Ok(vec![]);
        }

        let mut parent_addresses: Vec<ArkAddress> = vec![];

        for parent_checkpoint_outpoint in parent_checkoints {
            let parent_checkpoint_txid = parent_checkpoint_outpoint.txid.to_string();
            let parent_checkpoint_psbt = self
                .grpc_client
                .get_virtual_txs(vec![parent_checkpoint_txid.clone()], None)
                .await?;
            tracing::trace!(
                parent_checkpoint_txid = parent_checkpoint_txid,
                "Received checkpoint tx"
            );
            debug_assert!(parent_checkpoint_psbt.txs.len() == 1);
            let checkpoint_tx = parent_checkpoint_psbt.txs.first();

            match checkpoint_tx {
                None => {
                    tracing::error!("Checkpoint tx didn't have a parent")
                }
                Some(parent) => {
                    debug_assert!(parent.inputs.len() == 1);
                    let option = parent
                        .inputs
                        .first()
                        .context("No parent found")?
                        .witness_utxo
                        .clone();
                    let txout =
                        option.ok_or_else(|| ark_core::Error::ad_hoc("Could not find input"))?;
                    let server_x_only = self.server_info.signer_pk.x_only_public_key();
                    let buf = &txout.script_pubkey;
                    let ark_address =
                        get_address_from_output(buf, server_x_only.0, self.server_info.network)
                            .await;

                    if let Some(address) = ark_address {
                        let address_str = address.encode();
                        if !parent_addresses
                            .iter()
                            .any(|addr| addr.encode() == address_str)
                        {
                            parent_addresses.push(address);
                        }
                    }
                }
            }
        }

        Ok(parent_addresses)
    }

    pub fn get_game_addresses(&self) -> Vec<(GameType, Multiplier, ArkAddress)> {
        let vec = self.game_addresses.clone();
        vec.iter()
            .map(|a| (a.game_type, a.multiplier, a.vtxo.to_ark_address()))
            .collect()
    }

    /// Legacy method for backward compatibility
    pub fn get_game_addresses_legacy(&self) -> Vec<(Multiplier, ArkAddress)> {
        let vec = self.game_addresses.clone();
        vec.iter()
            .map(|a| (a.multiplier, a.vtxo.to_ark_address()))
            .collect()
    }

    pub fn dust_value(&self) -> Amount {
        self.server_info.dust
    }

    /// Find the game type and multiplier for a given address
    pub fn find_game_info(&self, address: &ArkAddress) -> Option<(GameType, Multiplier)> {
        self.game_addresses
            .iter()
            .find(|game_addr| game_addr.vtxo.to_ark_address().encode() == address.encode())
            .map(|game_addr| (game_addr.game_type, game_addr.multiplier))
    }

    /// Subscribe to script pubkeys for real-time notifications
    pub async fn subscribe_to_scripts(&self, scripts: Vec<ArkAddress>) -> Result<String> {
        let length = scripts.len();
        let subscription_id = self.grpc_client.subscribe_to_scripts(scripts, None).await?;
        tracing::info!(
            subscription_id = subscription_id,
            scripts = length,
            "📡 Subscribed scripts"
        );
        Ok(subscription_id)
    }

    /// Unsubscribe from script pubkeys
    pub async fn unsubscribe_from_scripts(
        &self,
        scripts: Vec<ArkAddress>,
        subscription_id: String,
    ) -> Result<()> {
        self.grpc_client
            .unsubscribe_from_scripts(scripts, subscription_id.clone())
            .await?;
        tracing::info!(
            subscription_id = subscription_id,
            "📡 Unsubscribed from scripts (placeholder implementation)"
        );
        Ok(())
    }

    /// Get subscription stream
    pub async fn get_subscription(
        &self,
        subscription_id: String,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = Result<SubscriptionEvent>> + Send + '_>>>
    {
        use futures::stream::StreamExt;

        let mut subscription_stream = self.grpc_client.get_subscription(subscription_id).await?;

        let game_addresses = self.get_game_addresses();

        let stream = async_stream::stream! {
            while let Some(result) = subscription_stream.next().await {
                match result {
                    Ok(SubscriptionResponse::Event(response)) => {
                        // Get the transaction details
                        let psbt = if let Some(psbt) = response.tx {
                            psbt
                        } else {
                            match self.grpc_client
                                .get_virtual_txs(vec![response.txid.to_string()], None)
                                .await {
                                Ok(fetched) => {
                                    if let Some(tx) = fetched.txs.into_iter().next() {
                                        tx
                                    } else {
                                        tracing::warn!("No transactions found for txid: {}", response.txid);
                                        continue;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to fetch transaction: {}", e);
                                    continue;
                                }
                            }
                        };

                        let tx = &psbt.unsigned_tx;
                        let txid = tx.compute_txid();

                        // Check each output to see if it matches one of our game addresses
                        for (vout, output) in tx.output.iter().enumerate() {
                            for (_, _, address) in &game_addresses {
                                if output.script_pubkey == address.to_p2tr_script_pubkey() {
                                    yield Ok(SubscriptionEvent {
                                        txid,
                                        vout: vout as u32,
                                        amount: Amount::from_sat(output.value.to_sat()),
                                        script_pubkey: output.script_pubkey.clone(),
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error receiving subscription response: {}", e);
                        yield Err(anyhow::anyhow!("Subscription error: {}", e));
                    }
                Ok(SubscriptionResponse::Heartbeat) => {}}
            }
        };

        Ok(Box::pin(stream))
    }
}

#[derive(Debug, Clone)]
pub struct GameArkAddress {
    pub game_type: GameType,
    pub multiplier: Multiplier,
    pub vtxo: Vtxo,
    pub secret_key: SecretKey,
}

async fn get_address_from_output(
    script: &bitcoin::ScriptBuf,
    server_pk: XOnlyPublicKey,
    network: bitcoin::Network,
) -> Option<ArkAddress> {
    let script = script.as_script();
    let instruction = script.instructions();
    let mut enumerate = instruction.enumerate();
    let (_, res) = enumerate.nth(1).expect("No more instructions");
    let instruction = res.expect("to be correct");
    match instruction {
        bitcoin::script::Instruction::PushBytes(b) => {
            let vtxo_tap_key =
                XOnlyPublicKey::from_slice(b.as_bytes()).expect("to have x-only-public key");
            let vtxo_tap_key = TweakedPublicKey::dangerous_assume_tweaked(vtxo_tap_key);
            let address = ArkAddress::new(network, server_pk, vtxo_tap_key);
            Some(address)
        }
        bitcoin::script::Instruction::Op(o) => {
            tracing::debug!("Opcode {o}");
            None
        }
    }
}
