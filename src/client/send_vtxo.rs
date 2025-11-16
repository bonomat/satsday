use crate::ArkClient;
use anyhow::Context;
use anyhow::Result;
use ark_core::coin_select::select_vtxos;
use ark_core::send;
use ark_core::send::build_offchain_transactions;
use ark_core::send::sign_ark_transaction;
use ark_core::send::sign_checkpoint_transaction;
use ark_core::send::OffchainTransactions;
use ark_core::ArkAddress;
use bitcoin::psbt;
use bitcoin::secp256k1;
use bitcoin::secp256k1::schnorr;
use bitcoin::Amount;
use bitcoin::Txid;
use bitcoin::XOnlyPublicKey;

impl ArkClient {
    /// Spend confirmed and pre-confimed VTXOs in an Ark transaction sending the given `amount` to
    /// the given `address`.
    ///
    /// The Ark transaction is built in collaboration with the Ark server. The outputs of said
    /// transaction will be pre-confirmed VTXOs.
    ///
    /// # Returns
    ///
    /// The [`Txid`] of the generated Ark transaction.
    pub async fn send_vtxo(&self, address: ArkAddress, amount: Amount) -> Result<Txid> {
        // Use cached spendable VTXOs instead of fetching
        let spendable_vtxos = self
            .get_cached_spendable_vtxos()
            .await
            .context("failed to get cached spendable VTXOs")?;

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

        let selected_coins = select_vtxos(
            spendable_virtual_tx_outpoints,
            amount,
            self.server_info.dust,
            true,
        )
        .context("failed to select coins")?;

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
                    .expect("to find matching default VTXO");

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
            &[(&address, amount)],
            Some(&change_address),
            &vtxo_inputs,
            &self.server_info,
        )
        .context("failed to build offchain transactions")?;

        let mut all_keys = vec![self.main_address.clone()];
        for game_address in &self.game_addresses {
            all_keys.push((game_address.vtxo.clone(), game_address.secret_key));
        }

        let sign_fn = |_psbt: &mut psbt::Input,
                       msg: secp256k1::Message,
                       index: usize|
         -> Result<(schnorr::Signature, XOnlyPublicKey), ark_core::Error> {
            let input = vtxo_inputs.get(index).expect("input");
            let kp = all_keys.iter().find_map(|(v, sk)| {
                if input.script_pubkey() == v.script_pubkey() {
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

        for i in 0..checkpoint_txs.len() {
            sign_ark_transaction(|a, b| sign_fn(a, b, i), &mut ark_tx, i)?;
        }

        let ark_txid = ark_tx.unsigned_tx.compute_txid();

        let mut res = self
            .grpc_client
            .submit_offchain_transaction_request(ark_tx, checkpoint_txs)
            .await
            .context("failed to submit offchain transaction request")?;

        for checkpoint_psbt in res.signed_checkpoint_txs.iter_mut() {
            sign_checkpoint_transaction(|a, b| sign_fn(a, b, 0), checkpoint_psbt)?;
        }

        self.grpc_client
            .finalize_offchain_transaction(ark_txid, res.signed_checkpoint_txs)
            .await
            .context("failed to finalize offchain transaction")?;

        Ok(ark_txid)
    }
}
