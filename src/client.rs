use anyhow::{Context, Result, bail};
use ark_core::{
    ArkAddress, BoardingOutput, TxGraph, Vtxo,
    boarding_output::{BoardingOutpoints, list_boarding_outpoints},
    coin_select::select_vtxos,
    proof_of_funds,
    redeem::{
        OffchainTransactions, VtxoInput, build_offchain_transactions, sign_checkpoint_transaction,
        sign_offchain_virtual_transaction,
    },
    round::{
        OnChainInput, VtxoInput as RoundVtxoInput, create_and_sign_forfeit_txs,
        generate_nonce_tree, sign_round_psbt, sign_vtxo_tree,
    },
    server::{BatchTreeEventType, RoundStreamEvent},
    vtxo::{VirtualTxOutpoints, list_virtual_tx_outpoints},
};
use bitcoin::key::TweakedPublicKey;
use bitcoin::{
    Amount, OutPoint, TxOut, Txid, XOnlyPublicKey,
    hashes::{Hash, sha256},
    hex::DisplayHex,
    key::{Keypair, Secp256k1},
    secp256k1::{self, PublicKey, SecretKey, schnorr},
};
use futures::StreamExt;
use rand::thread_rng;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use tokio::task::block_in_place;

use crate::{config::Config, esplora::EsploraClient};

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

impl ArkClient {
    pub async fn new(
        config: Config,
        main_sk: SecretKey,
        seed_1_5x_sk: SecretKey,
        seed_2x_sk: SecretKey,
    ) -> Result<Self> {
        let secp = Secp256k1::new();
        let main_pk = PublicKey::from_secret_key(&secp, &main_sk);
        let seed_1_5x_pk = PublicKey::from_secret_key(&secp, &seed_1_5x_sk);
        let seed_2x_pk = PublicKey::from_secret_key(&secp, &seed_2x_sk);

        let mut grpc_client = ark_grpc::Client::new(config.ark_server_url);
        grpc_client.connect().await?;

        let server_info = grpc_client.get_info().await?;
        let esplora_client = EsploraClient::new(&config.esplora_url)?;

        let main_vtxo = Vtxo::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            main_pk.x_only_public_key().0,
            vec![],
            server_info.unilateral_exit_delay,
            server_info.network,
        )?;
        let seed_1_5x_vtxo = Vtxo::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            seed_1_5x_pk.x_only_public_key().0,
            vec![],
            server_info.unilateral_exit_delay,
            server_info.network,
        )?;
        let seed_2x_pk_vtxo = Vtxo::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            seed_2x_pk.x_only_public_key().0,
            vec![],
            server_info.unilateral_exit_delay,
            server_info.network,
        )?;

        let boarding_output = BoardingOutput::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            main_pk.x_only_public_key().0,
            server_info.boarding_exit_delay,
            server_info.network,
        )?;

        Ok(Self {
            grpc_client,
            esplora_client,
            server_info,
            main_address: (main_vtxo, main_sk),
            game_addresses: vec![
                GameArkAddress {
                    multiplier: Multiplier::X15,
                    vtxo: seed_1_5x_vtxo,
                    secret_key: seed_1_5x_sk,
                },
                GameArkAddress {
                    multiplier: Multiplier::X2,
                    vtxo: seed_2x_pk_vtxo,
                    secret_key: seed_2x_sk,
                },
            ],
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

    pub async fn send(&self, address: &ArkAddress, amount: Amount) -> Result<Txid> {
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

        let vtxo_outpoints = virtual_tx_outpoints
            .spendable
            .iter()
            .map(|(outpoint, _)| ark_core::coin_select::VtxoOutPoint {
                outpoint: outpoint.outpoint,
                expire_at: outpoint.expires_at,
                amount: outpoint.amount,
            })
            .collect::<Vec<_>>();

        let selected_outpoints = if amount == Amount::ZERO {
            vtxo_outpoints
        } else {
            select_vtxos(vtxo_outpoints, amount, self.server_info.dust, true)?
        };

        // Calculate the actual amount to send (total of selected outpoints when amount is 0)
        let send_amount = if amount == Amount::ZERO {
            selected_outpoints.iter().map(|o| o.amount).sum()
        } else {
            amount
        };

        self.send_with_outpoints(address, Some(send_amount), &selected_outpoints)
            .await
    }

    pub async fn send_with_outpoints(
        &self,
        address: &ArkAddress,
        amount: Option<Amount>,
        specific_outpoints: &[ark_core::coin_select::VtxoOutPoint],
    ) -> Result<Txid> {
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

        let vtxo_inputs = virtual_tx_outpoints
            .spendable
            .into_iter()
            .filter(|(outpoint, _)| {
                specific_outpoints
                    .iter()
                    .any(|o| o.outpoint == outpoint.outpoint)
            })
            .map(|(outpoint, vtxo)| VtxoInput::new(vtxo, outpoint.amount, outpoint.outpoint))
            .collect::<Vec<_>>();

        let (main_address, _) = &self.main_address;
        let change_address = main_address.to_ark_address();

        // Calculate the amount to send: either the provided amount or sum of all outpoints
        let send_amount =
            amount.unwrap_or_else(|| specific_outpoints.iter().map(|o| o.amount).sum());

        let OffchainTransactions {
            mut virtual_tx,
            checkpoint_txs,
        } = build_offchain_transactions(
            &[(address, send_amount)],
            Some(&change_address),
            &vtxo_inputs,
            self.server_info.dust,
        )?;

        let mut all_keys = vec![self.main_address.clone()];
        for game_address in &self.game_addresses {
            all_keys.push((game_address.vtxo.clone(), game_address.secret_key));
        }

        let sign_fn = |msg: secp256k1::Message,
                       vtxo: &Vtxo|
         -> Result<(schnorr::Signature, XOnlyPublicKey), ark_core::Error> {
            // TODO: find the correct kp here, not sure how yet.
            let kp = all_keys.iter().find_map(|(v, sk)| {
                if v.to_ark_address().encode() == vtxo.to_ark_address().encode() {
                    Some(sk.keypair(&self.secp))
                } else {
                    None
                }
            });
            let kp = kp
                .context("Key not found for vtxo")
                .map_err(ark_core::Error::ad_hoc)?;

            let sig = self.secp.sign_schnorr_no_aux_rand(&msg, &kp);
            let pk = kp.x_only_public_key().0;
            Ok((sig, pk))
        };

        for (i, (_, _, _, vtxo)) in checkpoint_txs.iter().enumerate() {
            sign_offchain_virtual_transaction(
                |msg| sign_fn(msg, vtxo),
                &mut virtual_tx,
                &checkpoint_txs
                    .iter()
                    .map(|(_, output, outpoint, _)| (output.clone(), *outpoint))
                    .collect::<Vec<_>>(),
                i,
            )?;
        }

        let virtual_txid = virtual_tx.unsigned_tx.compute_txid();

        let mut res = self
            .grpc_client
            .submit_offchain_transaction_request(
                virtual_tx,
                checkpoint_txs
                    .into_iter()
                    .map(|(psbt, _, _, _)| psbt)
                    .collect(),
            )
            .await
            .context("failed to submit offchain transaction request")?;

        for checkpoint_psbt in res.signed_checkpoint_txs.iter_mut() {
            let vtxo_input = vtxo_inputs
                .iter()
                .find(|input| {
                    checkpoint_psbt.unsigned_tx.input[0].previous_output == input.outpoint()
                })
                .with_context(|| {
                    format!(
                        "could not find VTXO input for checkpoint transaction {}",
                        checkpoint_psbt.unsigned_tx.compute_txid(),
                    )
                })?;

            sign_checkpoint_transaction(
                |msg| sign_fn(msg, vtxo_input.vtxo()),
                checkpoint_psbt,
                vtxo_input,
            )?;
        }

        self.grpc_client
            .finalize_offchain_transaction(virtual_txid, res.signed_checkpoint_txs)
            .await
            .context("failed to finalize offchain transaction")?;

        Ok(virtual_txid)
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
    ) -> Result<HashMap<Vtxo, Vec<ark_core::server::VtxoOutPoint>>> {
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

    pub async fn spendable_game_vtxos(
        &self,
        select_recoverable_vtxos: bool,
    ) -> Result<HashMap<Vtxo, Vec<ark_core::server::VtxoOutPoint>>> {
        let mut spendable_vtxos = HashMap::new();

        for game_address in &self.game_addresses {
            let spendable = self
                ._spendable_vtxos(game_address.vtxo.clone(), select_recoverable_vtxos)
                .await?;
            spendable_vtxos.insert(spendable.0, spendable.1);
        }

        Ok(spendable_vtxos)
    }

    pub async fn _spendable_vtxos(
        &self,
        vtxo: Vtxo,
        select_recoverable_vtxos: bool,
    ) -> Result<(Vtxo, Vec<ark_core::server::VtxoOutPoint>)> {
        let vtxo_outpoints = self.grpc_client.list_vtxos(&vtxo.to_ark_address()).await?;

        let spendable = if select_recoverable_vtxos {
            vtxo_outpoints.spendable_with_recoverable()
        } else {
            vtxo_outpoints.spendable().to_vec()
        };

        Ok((vtxo, spendable))
    }

    async fn settle_internal(
        &self,
        vtxos: VirtualTxOutpoints,
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
                    proof_of_funds::Input::new(
                        outpoint,
                        boarding_output.exit_delay(),
                        TxOut {
                            value: amount,
                            script_pubkey: boarding_output.script_pubkey(),
                        },
                        boarding_output.tapscripts(),
                        boarding_output.owner_pk(),
                        boarding_output.exit_spend_info(),
                        true,
                    )
                },
            );

            let vtxo_inputs =
                vtxos
                    .spendable
                    .clone()
                    .into_iter()
                    .map(|(virtual_tx_outpoint, vtxo)| {
                        proof_of_funds::Input::new(
                            virtual_tx_outpoint.outpoint,
                            vtxo.exit_delay(),
                            TxOut {
                                value: virtual_tx_outpoint.amount,
                                script_pubkey: vtxo.script_pubkey(),
                            },
                            vtxo.tapscripts(),
                            vtxo.owner_pk(),
                            vtxo.exit_spend_info(),
                            false,
                        )
                    });

            boarding_inputs.chain(vtxo_inputs).collect::<Vec<_>>()
        };
        let n_round_inputs = round_inputs.len();

        let spendable_amount = boarding_outputs.spendable_balance() + vtxos.spendable_balance();
        let round_outputs = vec![proof_of_funds::Output::Offchain(TxOut {
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

        let (bip322_proof, intent_message) = proof_of_funds::make_bip322_signature(
            signing_kps.as_slice(),
            sign_for_onchain_pk_fn,
            round_inputs,
            round_outputs,
            own_cosigner_pks.clone(),
        )?;

        let intent_id = self
            .grpc_client
            .register_intent(&intent_message, &bip322_proof)
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
            Some(Ok(RoundStreamEvent::BatchStarted(e))) => e,
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
                Some(Ok(RoundStreamEvent::TreeTx(e))) => match e.batch_tree_event_type {
                    BatchTreeEventType::Vtxo => vtxo_graph_chunks.push(e.tx_graph_chunk),
                    BatchTreeEventType::Connector => {
                        bail!("Unexpected connector batch tree event");
                    }
                },
                Some(Ok(RoundStreamEvent::TreeSigningStarted(e))) => {
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
            &round_signing_event.unsigned_round_tx,
        )?;

        self.grpc_client
            .submit_tree_nonces(
                &round_id,
                cosigner_kp.public_key(),
                nonce_tree.to_nonce_pks(),
            )
            .await?;

        let round_signing_nonces_generated_event = match event_stream.next().await {
            Some(Ok(RoundStreamEvent::TreeNoncesAggregated(e))) => e,
            other => bail!("Did not get round signing nonces generated event: {other:?}"),
        };

        let round_id = round_signing_nonces_generated_event.id;
        let agg_pub_nonce_tree = round_signing_nonces_generated_event.tree_nonces;

        let partial_sig_tree = sign_vtxo_tree(
            self.server_info.vtxo_tree_expiry,
            self.server_info.pk.x_only_public_key().0,
            &cosigner_kp,
            &vtxo_graph,
            &round_signing_event.unsigned_round_tx,
            nonce_tree,
            &agg_pub_nonce_tree,
        )?;

        self.grpc_client
            .submit_tree_signatures(&round_id, cosigner_kp.public_key(), partial_sig_tree)
            .await?;

        let mut connectors_graph_chunks = Vec::new();

        let round_finalization_event;
        loop {
            match event_stream.next().await {
                Some(Ok(RoundStreamEvent::TreeTx(e))) => match e.batch_tree_event_type {
                    BatchTreeEventType::Vtxo => {
                        bail!("Unexpected VTXO batch tree event");
                    }
                    BatchTreeEventType::Connector => {
                        connectors_graph_chunks.push(e.tx_graph_chunk);
                    }
                },
                Some(Ok(RoundStreamEvent::TreeSignature(e))) => match e.batch_tree_event_type {
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
                Some(Ok(RoundStreamEvent::BatchFinalization(e))) => {
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
                RoundVtxoInput::new(
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
                OnChainInput::new(boarding_output, amount, outpoint)
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

            sign_round_psbt(sign_for_pk_fn, &mut round_psbt, &onchain_inputs)?;

            Some(round_psbt)
        };

        self.grpc_client
            .submit_signed_forfeit_txs(signed_forfeit_psbts, round_psbt)
            .await?;

        let round_finalized_event = match event_stream.next().await {
            Some(Ok(RoundStreamEvent::BatchFinalized(e))) => e,
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
                    let option = parent.inputs.first().unwrap().witness_utxo.clone();
                    let txout =
                        option.ok_or_else(|| ark_core::Error::ad_hoc("Could not find input"))?;
                    let server_x_only = self.server_info.pk.x_only_public_key();
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

    pub fn get_game_addresses(&self) -> Vec<(Multiplier, ArkAddress)> {
        let vec = self.game_addresses.clone();
        vec.iter()
            .map(|a| (a.multiplier, a.vtxo.to_ark_address()))
            .collect()
    }
}

#[derive(Debug, Clone)]
struct GameArkAddress {
    multiplier: Multiplier,
    vtxo: Vtxo,
    secret_key: SecretKey,
}

#[derive(Debug, Clone, Copy)]
pub enum Multiplier {
    X15,
    X2,
}

impl Display for Multiplier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Multiplier::X15 => write!(f, "X15"),
            Multiplier::X2 => write!(f, "X2"),
        }
    }
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
    let instruction = res.unwrap();
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
