use anyhow::{bail, Context, Result};
use ark_core::{
    boarding_output::{list_boarding_outpoints, BoardingOutpoints},
    coin_select::select_vtxos,
    proof_of_funds,
    redeem::{build_offchain_transactions, sign_checkpoint_transaction, sign_offchain_virtual_transaction, OffchainTransactions, VtxoInput},
    round::{create_and_sign_forfeit_txs, generate_nonce_tree, sign_round_psbt, sign_vtxo_tree, OnChainInput, VtxoInput as RoundVtxoInput},
    server::{BatchTreeEventType, RoundStreamEvent},
    vtxo::{list_virtual_tx_outpoints, VirtualTxOutpoints},
    ArkAddress, ArkTransaction, BoardingOutput, TxGraph, Vtxo,
};
use bitcoin::{
    hashes::{sha256, Hash},
    hex::DisplayHex,
    key::{Keypair, Secp256k1},
    secp256k1::{self, schnorr, PublicKey, SecretKey},
    Amount, TxOut, Txid, XOnlyPublicKey,
};
use futures::StreamExt;
use rand::thread_rng;
use std::collections::HashMap;
use tokio::task::block_in_place;

use crate::{config::Config, esplora::EsploraClient};

pub struct ArkClient {
    grpc_client: ark_grpc::Client,
    esplora_client: EsploraClient,
    secret_key: SecretKey,
    public_key: PublicKey,
    server_info: ark_core::server::Info,
    vtxo: Vtxo,
    boarding_output: BoardingOutput,
    secp: Secp256k1<secp256k1::All>,
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
    pub async fn new(config: Config, secret_key: SecretKey) -> Result<Self> {
        let secp = Secp256k1::new();
        let public_key = PublicKey::from_secret_key(&secp, &secret_key);

        let mut grpc_client = ark_grpc::Client::new(config.ark_server_url);
        grpc_client.connect().await?;

        let server_info = grpc_client.get_info().await?;
        let esplora_client = EsploraClient::new(&config.esplora_url)?;

        let vtxo = Vtxo::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            public_key.x_only_public_key().0,
            vec![],
            server_info.unilateral_exit_delay,
            server_info.network,
        )?;

        let boarding_output = BoardingOutput::new(
            &secp,
            server_info.pk.x_only_public_key().0,
            public_key.x_only_public_key().0,
            server_info.boarding_exit_delay,
            server_info.network,
        )?;

        Ok(Self {
            grpc_client,
            esplora_client,
            secret_key,
            public_key,
            server_info,
            vtxo,
            boarding_output,
            secp,
        })
    }

    pub fn get_address(&self) -> ArkAddress {
        self.vtxo.to_ark_address()
    }

    pub fn get_boarding_address(&self) -> bitcoin::Address {
        self.boarding_output.address().clone()
    }

    pub async fn get_balance(&self) -> Result<Balance> {
        let runtime = tokio::runtime::Handle::current();
        let find_outpoints_fn = |address: &bitcoin::Address| -> Result<Vec<ark_core::ExplorerUtxo>, ark_core::Error> {
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

        let boarding_outpoints = list_boarding_outpoints(find_outpoints_fn, &[self.boarding_output.clone()])?;

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
        let find_outpoints_fn = |address: &bitcoin::Address| -> Result<Vec<ark_core::ExplorerUtxo>, ark_core::Error> {
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

        let selected_outpoints = select_vtxos(vtxo_outpoints, amount, self.server_info.dust, true)?;

        let vtxo_inputs = virtual_tx_outpoints
            .spendable
            .into_iter()
            .filter(|(outpoint, _)| {
                selected_outpoints
                    .iter()
                    .any(|o| o.outpoint == outpoint.outpoint)
            })
            .map(|(outpoint, vtxo)| VtxoInput::new(vtxo, outpoint.amount, outpoint.outpoint))
            .collect::<Vec<_>>();

        let change_address = self.vtxo.to_ark_address();
        let kp = Keypair::from_secret_key(&self.secp, &self.secret_key);

        let OffchainTransactions {
            mut virtual_tx,
            checkpoint_txs,
        } = build_offchain_transactions(
            &[(address, amount)],
            Some(&change_address),
            &vtxo_inputs,
            self.server_info.dust,
        )?;

        let sign_fn = |msg: secp256k1::Message| -> Result<(schnorr::Signature, XOnlyPublicKey), ark_core::Error> {
            let sig = self.secp.sign_schnorr_no_aux_rand(&msg, &kp);
            let pk = kp.x_only_public_key().0;
            Ok((sig, pk))
        };

        for i in 0..checkpoint_txs.len() {
            sign_offchain_virtual_transaction(
                sign_fn,
                &mut virtual_tx,
                &checkpoint_txs
                    .iter()
                    .map(|(_, output, outpoint)| (output.clone(), *outpoint))
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
                    .map(|(psbt, _, _)| psbt)
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

            sign_checkpoint_transaction(sign_fn, checkpoint_psbt, vtxo_input)?;
        }

        self.grpc_client
            .finalize_offchain_transaction(virtual_txid, res.signed_checkpoint_txs)
            .await
            .context("failed to finalize offchain transaction")?;

        Ok(virtual_txid)
    }

    pub async fn settle(&self) -> Result<Option<Txid>> {
        let runtime = tokio::runtime::Handle::current();
        let find_outpoints_fn = |address: &bitcoin::Address| -> Result<Vec<ark_core::ExplorerUtxo>, ark_core::Error> {
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

        let boarding_outpoints = list_boarding_outpoints(find_outpoints_fn, &[self.boarding_output.clone()])?;

        self.settle_internal(virtual_tx_outpoints, boarding_outpoints).await
    }

    pub async fn transaction_history(&self) -> Result<Vec<ArkTransaction>> {
        let boarding_addresses = vec![self.boarding_output.address().clone()];
        let vtxos = vec![self.vtxo.clone()];

        let mut boarding_transactions = Vec::new();
        let mut boarding_round_transactions = Vec::new();

        for boarding_address in boarding_addresses.iter() {
            let outpoints = self.esplora_client.find_outpoints(boarding_address).await?;

            for utxo in outpoints.iter() {
                let confirmed_at = utxo.confirmation_blocktime.map(|t| t as i64);

                boarding_transactions.push(ArkTransaction::Boarding {
                    txid: utxo.outpoint.txid,
                    amount: utxo.amount,
                    confirmed_at,
                });

                let status = self
                    .esplora_client
                    .get_output_status(&utxo.outpoint.txid, utxo.outpoint.vout)
                    .await?;

                if let Some(spend_txid) = status.spend_txid {
                    boarding_round_transactions.push(spend_txid);
                }
            }
        }

        let mut offchain_transactions = Vec::new();
        for vtxo in vtxos.iter() {
            let txs = self.grpc_client.get_tx_history(&vtxo.to_ark_address()).await?;

            for tx in txs {
                if !boarding_round_transactions.contains(&tx.txid()) {
                    offchain_transactions.push(tx);
                }
            }
        }

        let mut txs = [boarding_transactions, offchain_transactions].concat();
        ark_core::sort_transactions_by_created_at(&mut txs);

        Ok(txs)
    }

    pub async fn spendable_vtxos(&self, select_recoverable_vtxos: bool) -> Result<HashMap<Vtxo, Vec<ark_core::server::VtxoOutPoint>>> {
        let mut spendable_vtxos = HashMap::new();
        let vtxo_outpoints = self.grpc_client.list_vtxos(&self.vtxo.to_ark_address()).await?;

        let spendable = if select_recoverable_vtxos {
            vtxo_outpoints.spendable_with_recoverable()
        } else {
            vtxo_outpoints.spendable().to_vec()
        };

        spendable_vtxos.insert(self.vtxo.clone(), spendable);
        Ok(spendable_vtxos)
    }

    async fn settle_internal(&self, vtxos: VirtualTxOutpoints, boarding_outputs: BoardingOutpoints) -> Result<Option<Txid>> {
        let mut rng = thread_rng();

        if vtxos.spendable.is_empty() && boarding_outputs.spendable.is_empty() {
            return Ok(None);
        }

        let cosigner_kp = Keypair::new(&self.secp, &mut rng);
        let to_address = self.vtxo.to_ark_address();

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

            let vtxo_inputs = vtxos
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

        let signing_kp = Keypair::from_secret_key(&self.secp, &self.secret_key);
        let sign_for_onchain_pk_fn = |_: &XOnlyPublicKey,
                                      msg: &secp256k1::Message|
         -> Result<schnorr::Signature, ark_core::Error> {
            Ok(self.secp.sign_schnorr_no_aux_rand(msg, &signing_kp))
        };

        let (bip322_proof, intent_message) = proof_of_funds::make_bip322_signature(
            &signing_kp,
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
            self.grpc_client.confirm_registration(intent_id.clone()).await?;
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

        let round_id = round_finalization_event.id;

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
                &signing_kp,
                vtxo_inputs.as_slice(),
                &connectors_graph.leaves(),
                &self.server_info.forfeit_address,
                self.server_info.dust,
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
                Ok(self.secp.sign_schnorr_no_aux_rand(msg, &signing_kp))
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
}