mod send_vtxo;
mod settle;

use crate::config::Config;
use crate::esplora::EsploraClient;
use crate::games::GameType;
use crate::key_derivation::KeyDerivation;
use crate::key_derivation::Multiplier;
use anyhow::Context;
use anyhow::Result;
use ark_core::boarding_output::list_boarding_outpoints;
use ark_core::server::GetVtxosRequest;
use ark_core::server::SubscriptionResponse;
use ark_core::vtxo::list_virtual_tx_outpoints;
use ark_core::ArkAddress;
use ark_core::BoardingOutput;
use ark_core::Vtxo;
use bitcoin::key::Secp256k1;
use bitcoin::key::TweakedPublicKey;
use bitcoin::secp256k1::PublicKey;
use bitcoin::secp256k1::SecretKey;
use bitcoin::secp256k1::{self};
use bitcoin::Amount;
use bitcoin::OutPoint;
use bitcoin::Txid;
use bitcoin::XOnlyPublicKey;
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

        let boarding_outpoints = list_boarding_outpoints(
            find_outpoints_fn,
            std::slice::from_ref(&self.boarding_output),
        )?;

        Ok(Balance {
            offchain_spendable: virtual_tx_outpoints.spendable_balance(),
            offchain_expired: virtual_tx_outpoints.expired_balance(),
            boarding_spendable: boarding_outpoints.spendable_balance(),
            boarding_expired: boarding_outpoints.expired_balance(),
            boarding_pending: boarding_outpoints.pending_balance(),
        })
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
            let spendable = vtxo_outpoints.spendable();
            spendable
                .into_iter()
                .filter(|v| !v.is_recoverable())
                .cloned()
                .collect::<Vec<_>>()
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
                        
                        let new_vtxos = response.new_vtxos;

                        for new_vtxo in new_vtxos {
                            for (_, _, address) in &game_addresses {
                                if new_vtxo.clone().script == address.to_sub_dust_script_pubkey() ||
                                new_vtxo.clone().script == address.to_p2tr_script_pubkey(){
                                    yield Ok(SubscriptionEvent {
                                        txid: new_vtxo.outpoint.txid,
                                        vout: new_vtxo.outpoint.vout,
                                        amount: new_vtxo.amount,
                                        script_pubkey: new_vtxo.script.clone(),
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
