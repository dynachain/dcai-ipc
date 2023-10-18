// Copyright 2022-2023 Protocol Labs
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use ethers::types::H256;
use ipc_actors_abis::{
    gateway_getter_facet, gateway_manager_facet, gateway_messenger_facet,
    subnet_actor_getter_facet, subnet_actor_manager_facet, subnet_registry,
};
use ipc_sdk::evm::{fil_to_eth_amount, payload_to_evm_address, subnet_id_to_evm_addresses};
use ipc_sdk::validator::from_contract_validators;
use ipc_sdk::{eth_to_fil_amount, ethers_address_to_fil_address};

use crate::config::subnet::SubnetConfig;
use crate::config::Subnet;
use crate::lotus::message::ipc::SubnetInfo;
use crate::manager::subnet::{
    BottomUpCheckpointRelayer, GetBlockHashResult, SubnetGenesisInfo, TopDownCheckpointQuery,
    TopDownQueryPayload,
};
use crate::manager::{EthManager, SubnetManager};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use ethers::abi::Tokenizable;
use ethers::prelude::k256::ecdsa::SigningKey;
use ethers::prelude::{Signer, SignerMiddleware};
use ethers::providers::{Authorization, Http, Middleware, Provider};
use ethers::signers::{LocalWallet, Wallet};
use ethers::types::{BlockId, Eip1559TransactionRequest, I256, U256};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::{address::Address, econ::TokenAmount};
use ipc_identity::{EthKeyAddress, EvmKeyStore, PersistentKeyStore};
use ipc_sdk::checkpoint::{BottomUpCheckpoint, BottomUpCheckpointBundle};
use ipc_sdk::cross::CrossMsg;
use ipc_sdk::gateway::Status;
use ipc_sdk::staking::{NewStakingRequest, StakingChangeRequest};
use ipc_sdk::subnet::ConstructParams;
use ipc_sdk::subnet_id::SubnetID;
use num_traits::ToPrimitive;
use std::result;

pub type DefaultSignerMiddleware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

/// Default polling time used by the Ethers provider to check for pending
/// transactions and events. Default is 7, and for our child subnets we
/// can reduce it to the block time (or potentially less)
const ETH_PROVIDER_POLLING_TIME: Duration = Duration::from_secs(1);
/// Maximum number of retries to fetch a transaction receipt.
/// The number of retries should ensure that for the block time
/// of the network the number of retires considering the polling
/// time above waits enough tie to get the transaction receipt.
/// We currently support a low polling time and high number of
/// retries so these numbers accommodate fast subnets with slow
/// roots (like Calibration and mainnet).
const TRANSACTION_RECEIPT_RETRIES: usize = 200;

/// The majority vote percentage for checkpoint submission when creating a subnet.
const SUBNET_MAJORITY_PERCENTAGE: u8 = 60;

pub struct EthSubnetManager {
    keystore: Option<Arc<RwLock<PersistentKeyStore<EthKeyAddress>>>>,
    ipc_contract_info: IPCContractInfo,
}

/// Keep track of the on chain information for the subnet manager
struct IPCContractInfo {
    gateway_addr: ethers::types::Address,
    registry_addr: ethers::types::Address,
    chain_id: u64,
    provider: Provider<Http>,
}

#[async_trait]
impl TopDownCheckpointQuery for EthSubnetManager {
    async fn genesis_epoch(&self, subnet_id: &SubnetID) -> Result<ChainEpoch> {
        let address = contract_address_from_subnet(subnet_id)?;
        log::info!("querying genesis epoch in evm subnet contract: {address:}");

        let evm_subnet_id = gateway_getter_facet::SubnetID::try_from(subnet_id)?;

        let contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let (exists, subnet) = contract.get_subnet(evm_subnet_id).call().await?;
        if !exists {
            return Err(anyhow!("subnet: {} does not exists", subnet_id));
        }
        Ok(subnet.genesis_epoch.as_u64() as ChainEpoch)
    }

    async fn chain_head_height(&self) -> Result<ChainEpoch> {
        let block = self
            .ipc_contract_info
            .provider
            .get_block_number()
            .await
            .context("cannot get evm block number")?;
        Ok(block.as_u64() as ChainEpoch)
    }

    async fn get_top_down_msgs(
        &self,
        subnet_id: &SubnetID,
        epoch: ChainEpoch,
        block_hash: &[u8],
    ) -> Result<Vec<CrossMsg>> {
        if block_hash.len() != 32 {
            return Err(anyhow!("invalid block hash len"));
        }

        let route = subnet_id_to_evm_addresses(subnet_id)?;
        log::debug!("getting top down messages for route: {route:?}");

        let subnet_id = gateway_getter_facet::SubnetID {
            root: subnet_id.root_id(),
            route,
        };
        let gateway_contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );

        let call = gateway_contract
            .get_top_down_msgs(subnet_id, U256::from(epoch))
            .block(BlockId::from(H256::from_slice(block_hash)));
        let raw_msgs = call
            .call()
            .await
            .map_err(|e| anyhow!("cannot get evm top down messages: {e:}"))?;

        let mut msgs = vec![];
        for c in raw_msgs {
            msgs.push(ipc_sdk::cross::CrossMsg::try_from(c)?);
        }
        Ok(msgs)
    }

    async fn get_block_hash(&self, height: ChainEpoch) -> Result<GetBlockHashResult> {
        let block = self
            .ipc_contract_info
            .provider
            .get_block(height as u64)
            .await?
            .ok_or_else(|| anyhow!("height does not exist"))?;

        Ok(GetBlockHashResult {
            parent_block_hash: block.parent_hash.to_fixed_bytes().to_vec(),
            block_hash: block
                .hash
                .ok_or_else(|| anyhow!("block hash is empty"))?
                .to_fixed_bytes()
                .to_vec(),
        })
    }

    async fn get_validator_changeset(
        &self,
        subnet_id: &SubnetID,
        epoch: ChainEpoch,
    ) -> Result<TopDownQueryPayload<Vec<StakingChangeRequest>>> {
        let address = contract_address_from_subnet(subnet_id)?;
        log::info!("querying validator changes in evm subnet contract: {address:}");

        let contract = subnet_actor_manager_facet::SubnetActorManagerFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );

        let ev = contract
            .event::<NewStakingRequest>()
            .from_block(epoch as u64)
            .to_block(epoch as u64);

        let mut changes = vec![];
        let mut hash = None;
        for (event, meta) in ev.query_with_meta().await? {
            if let Some(h) = hash {
                if h != meta.block_hash {
                    return Err(anyhow!("block hash not equal"));
                }
            } else {
                hash = Some(meta.block_hash);
            }
            changes.push(StakingChangeRequest::try_from(event)?);
        }

        let block_hash = if let Some(h) = hash {
            h.0.to_vec()
        } else {
            self.get_block_hash(epoch).await?.block_hash
        };
        Ok(TopDownQueryPayload {
            value: changes,
            block_hash,
        })
    }
}

#[async_trait]
impl SubnetManager for EthSubnetManager {
    async fn create_subnet(&self, from: Address, params: ConstructParams) -> Result<Address> {
        self.ensure_same_gateway(&params.ipc_gateway_addr)?;

        let min_validator_stake = params
            .min_validator_stake
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid min validator stake"))?;

        let min_cross_msg_fee = params
            .min_cross_msg_fee
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid min message fee"))?;

        log::debug!("calling create subnet for EVM manager");

        let route = subnet_id_to_evm_addresses(&params.parent)?;
        log::debug!("root SubnetID as Ethereum type: {route:?}");

        let params = subnet_registry::ConstructorParams {
            parent_id: subnet_registry::SubnetID {
                root: params.parent.root_id(),
                route,
            },
            ipc_gateway_addr: self.ipc_contract_info.gateway_addr,
            consensus: params.consensus as u64 as u8,
            min_activation_collateral: ethers::types::U256::from(min_validator_stake),
            min_validators: params.min_validators,
            bottom_up_check_period: params.bottomup_check_period as u64,
            majority_percentage: SUBNET_MAJORITY_PERCENTAGE,
            active_validators_limit: params.active_validators_limit,
            power_scale: 3,
            min_cross_msg_fee: ethers::types::U256::from(min_cross_msg_fee),
        };

        log::info!("creating subnet on evm with params: {params:?}");

        let signer = self.get_signer(&from)?;
        let signer = Arc::new(signer);
        let registry_contract = subnet_registry::SubnetRegistry::new(
            self.ipc_contract_info.registry_addr,
            signer.clone(),
        );

        let call = call_with_premium_estimation(signer, registry_contract.new_subnet_actor(params))
            .await?;
        // TODO: Edit call to get estimate premium
        let pending_tx = call.send().await?;
        // We need the retry to parse the deployment event. At the time of this writing, it's a bug
        // in current FEVM that without the retries, events are not picked up.
        // See https://github.com/filecoin-project/community/discussions/638 for more info and updates.
        let receipt = pending_tx.retries(TRANSACTION_RECEIPT_RETRIES).await?;
        match receipt {
            Some(r) => {
                for log in r.logs {
                    log::debug!("log: {log:?}");

                    match ethers_contract::parse_log::<subnet_registry::SubnetDeployedFilter>(log) {
                        Ok(subnet_deploy) => {
                            let subnet_registry::SubnetDeployedFilter { subnet_addr } =
                                subnet_deploy;

                            log::debug!("subnet deployed at {subnet_addr:?}");
                            return ethers_address_to_fil_address(&subnet_addr);
                        }
                        Err(_) => {
                            log::debug!("no event for subnet actor published yet, continue");
                            continue;
                        }
                    }
                }
                Err(anyhow!("no logs receipt"))
            }
            None => Err(anyhow!("no receipt for event, txn not successful")),
        }
    }

    async fn join_subnet(
        &self,
        subnet: SubnetID,
        from: Address,
        collateral: TokenAmount,
        pub_key: Vec<u8>,
    ) -> Result<()> {
        let collateral = collateral
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid min validator stake"))?;

        let address = contract_address_from_subnet(&subnet)?;
        log::info!(
            "interacting with evm subnet contract: {address:} with collateral: {collateral:}"
        );

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        let mut txn = contract.join(ethers::types::Bytes::from(pub_key));
        txn.tx.set_value(collateral);
        let txn = call_with_premium_estimation(signer, txn).await?;

        txn.send().await?.await?;

        Ok(())
    }

    async fn stake(&self, subnet: SubnetID, from: Address, collateral: TokenAmount) -> Result<()> {
        let collateral = collateral
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid min validator stake"))?;

        let address = contract_address_from_subnet(&subnet)?;
        log::info!(
            "interacting with evm subnet contract: {address:} with collateral: {collateral:}"
        );

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        let mut txn = contract.stake();
        txn.tx.set_value(collateral);
        let txn = call_with_premium_estimation(signer, txn).await?;

        txn.send().await?.await?;

        Ok(())
    }

    async fn leave_subnet(&self, subnet: SubnetID, from: Address) -> Result<()> {
        let address = contract_address_from_subnet(&subnet)?;
        log::info!("leaving evm subnet: {subnet:} at contract: {address:}");

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        call_with_premium_estimation(signer, contract.leave())
            .await?
            .send()
            .await?
            .await?;

        Ok(())
    }

    async fn kill_subnet(&self, subnet: SubnetID, from: Address) -> Result<()> {
        let address = contract_address_from_subnet(&subnet)?;
        log::info!("kill evm subnet: {subnet:} at contract: {address:}");

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        call_with_premium_estimation(signer, contract.kill())
            .await?
            .send()
            .await?
            .await?;

        Ok(())
    }

    async fn list_child_subnets(
        &self,
        gateway_addr: Address,
    ) -> Result<HashMap<SubnetID, SubnetInfo>> {
        self.ensure_same_gateway(&gateway_addr)?;

        let gateway_contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );

        let mut s = HashMap::new();

        let evm_subnets = gateway_contract.list_subnets().call().await?;
        log::debug!("raw subnet: {evm_subnets:?}");

        for subnet in evm_subnets {
            let info = SubnetInfo::try_from(subnet)?;
            s.insert(info.id.clone(), info);
        }

        Ok(s)
    }

    async fn claim_collateral(&self, subnet: SubnetID, from: Address) -> Result<()> {
        let address = contract_address_from_subnet(&subnet)?;
        log::info!("claim collateral evm subnet: {subnet:} at contract: {address:}");

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        call_with_premium_estimation(signer, contract.claim())
            .await?
            .send()
            .await?
            .await?;

        Ok(())
    }

    async fn claim_relayer_reward(&self, subnet: SubnetID, from: Address) -> Result<()> {
        let address = contract_address_from_subnet(&subnet)?;
        log::info!("claim relayer reward evm subnet: {subnet:} at contract: {address:}");

        let signer = Arc::new(self.get_signer(&from)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());

        call_with_premium_estimation(signer, contract.claim_reward_for_relayer())
            .await?
            .send()
            .await?
            .await?;

        Ok(())
    }

    async fn fund(
        &self,
        subnet: SubnetID,
        gateway_addr: Address,
        from: Address,
        to: Address,
        amount: TokenAmount,
    ) -> Result<ChainEpoch> {
        self.ensure_same_gateway(&gateway_addr)?;

        let value = amount
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid value to fund"))?;

        log::info!("fund with evm gateway contract: {gateway_addr:} with value: {value:}, original: {amount:?}");

        let evm_subnet_id = gateway_manager_facet::SubnetID::try_from(&subnet)?;
        log::debug!("evm subnet id to fund: {evm_subnet_id:?}");

        let signer = Arc::new(self.get_signer(&from)?);
        let gateway_contract = gateway_manager_facet::GatewayManagerFacet::new(
            self.ipc_contract_info.gateway_addr,
            signer.clone(),
        );

        let mut txn = gateway_contract.fund(
            evm_subnet_id,
            gateway_manager_facet::FvmAddress::try_from(to)?,
        );
        txn.tx.set_value(value);
        let txn = call_with_premium_estimation(signer, txn).await?;

        let pending_tx = txn.send().await?;
        let receipt = pending_tx.retries(TRANSACTION_RECEIPT_RETRIES).await?;
        block_number_from_receipt(receipt)
    }

    async fn release(
        &self,
        _subnet: SubnetID,
        gateway_addr: Address,
        from: Address,
        to: Address,
        amount: TokenAmount,
        fee: Option<TokenAmount>,
    ) -> Result<ChainEpoch> {
        self.ensure_same_gateway(&gateway_addr)?;

        let value = amount
            .atto()
            .to_u128()
            .ok_or_else(|| anyhow!("invalid value to fund"))?;

        let fee = match fee {
            Some(f) => fil_to_eth_amount(&f)?,
            None => {
                let gateway_getter = gateway_getter_facet::GatewayGetterFacet::new(
                    self.ipc_contract_info.gateway_addr,
                    Arc::new(self.ipc_contract_info.provider.clone()),
                );
                gateway_getter.cross_msg_fee().call().await?
            }
        };

        log::info!("release with evm gateway contract: {gateway_addr:} with value: {value:}");

        let signer = Arc::new(self.get_signer(&from)?);
        let gateway_contract = gateway_manager_facet::GatewayManagerFacet::new(
            self.ipc_contract_info.gateway_addr,
            signer.clone(),
        );
        let mut txn =
            gateway_contract.release(gateway_manager_facet::FvmAddress::try_from(to)?, fee);
        txn.tx.set_value(value);
        let txn = call_with_premium_estimation(signer, txn).await?;

        let pending_tx = txn.send().await?;
        let receipt = pending_tx.retries(TRANSACTION_RECEIPT_RETRIES).await?;
        block_number_from_receipt(receipt)
    }

    /// Propagate the postbox message key. The key should be `bytes32`.
    async fn propagate(
        &self,
        _subnet: SubnetID,
        gateway_addr: Address,
        from: Address,
        postbox_msg_key: Vec<u8>,
    ) -> Result<()> {
        if postbox_msg_key.len() != 32 {
            return Err(anyhow!(
                "invalid message cid length, expect 32 but found {}",
                postbox_msg_key.len()
            ));
        }

        self.ensure_same_gateway(&gateway_addr)?;

        log::info!("propagate postbox evm gateway contract: {gateway_addr:} with message key: {postbox_msg_key:?}");

        let signer = Arc::new(self.get_signer(&from)?);
        let gateway_contract = gateway_messenger_facet::GatewayMessengerFacet::new(
            self.ipc_contract_info.gateway_addr,
            signer.clone(),
        );

        let mut key = [0u8; 32];
        key.copy_from_slice(&postbox_msg_key);

        call_with_premium_estimation(signer, gateway_contract.propagate(key))
            .await?
            .send()
            .await?;

        Ok(())
    }

    async fn send_cross_message(
        &self,
        gateway_addr: Address,
        from: Address,
        cross_msg: CrossMsg,
    ) -> Result<()> {
        self.ensure_same_gateway(&gateway_addr)?;

        log::info!("send evm cross messages to gateway contract: {gateway_addr:} with message: {cross_msg:?}");

        let signer = Arc::new(self.get_signer(&from)?);
        let gateway_contract = gateway_messenger_facet::GatewayMessengerFacet::new(
            self.ipc_contract_info.gateway_addr,
            signer.clone(),
        );

        let evm_cross_msg = gateway_messenger_facet::CrossMsg::try_from(cross_msg)?;
        call_with_premium_estimation(signer, gateway_contract.send_cross_message(evm_cross_msg))
            .await?
            .send()
            .await?;

        Ok(())
    }

    /// Send value between two addresses in a subnet
    async fn send_value(&self, from: Address, to: Address, amount: TokenAmount) -> Result<()> {
        let signer = Arc::new(self.get_signer(&from)?);
        let (fee, fee_cap) = premium_estimation(signer.clone()).await?;
        let tx = Eip1559TransactionRequest::new()
            .to(payload_to_evm_address(to.payload())?)
            .value(fil_to_eth_amount(&amount)?)
            .max_priority_fee_per_gas(fee)
            .max_fee_per_gas(fee_cap);

        let tx_pending = signer.send_transaction(tx, None).await?;

        log::info!(
            "sending FIL from {from:} to {to:} in tx {:?}",
            tx_pending.tx_hash()
        );
        tx_pending.await?;
        Ok(())
    }

    async fn wallet_balance(&self, address: &Address) -> Result<TokenAmount> {
        let balance = self
            .ipc_contract_info
            .provider
            .clone()
            .get_balance(payload_to_evm_address(address.payload())?, None)
            .await?;
        Ok(TokenAmount::from_atto(balance.as_u128()))
    }

    async fn get_chain_id(&self) -> Result<String> {
        Ok(self
            .ipc_contract_info
            .provider
            .get_chainid()
            .await?
            .to_string())
    }

    async fn get_genesis_info(&self, subnet: &SubnetID) -> Result<SubnetGenesisInfo> {
        let address = contract_address_from_subnet(subnet)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        Ok(SubnetGenesisInfo {
            // Active validators limit set for the child subnet.
            active_validators_limit: contract.active_validators_limit().call().await?,
            // Bottom-up checkpoint period set in the subnet actor.
            bottom_up_checkpoint_period: contract.bottom_up_check_period().call().await?,
            // Genesis epoch when the subnet was bootstrapped in the parent.
            genesis_epoch: self.genesis_epoch(subnet).await?,
            // Majority percentage of
            majority_percentage: contract.majority_percentage().call().await?,
            // Minimum collateral required for subnets to register into the subnet
            min_collateral: eth_to_fil_amount(&contract.min_activation_collateral().call().await?)?,
            // Custom message fee that the child subnet wants to set for cross-net messages
            msg_fee: eth_to_fil_amount(&contract.min_cross_msg_fee().call().await?)?,
            validators: from_contract_validators(contract.genesis_validators().call().await?)?,
        })
    }
}

#[async_trait]
impl EthManager for EthSubnetManager {
    async fn current_epoch(&self) -> Result<ChainEpoch> {
        let block_number = self
            .ipc_contract_info
            .provider
            .get_block_number()
            .await?
            .as_u64();
        Ok(block_number as ChainEpoch)
    }

    async fn bottom_up_checkpoint(
        &self,
        epoch: ChainEpoch,
    ) -> Result<subnet_actor_manager_facet::BottomUpCheckpoint> {
        let gateway_contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let checkpoint = gateway_contract
            .bottom_up_checkpoint(epoch as u64)
            .call()
            .await?;
        log::debug!("raw bottom up checkpoint from gateway: {checkpoint:?}");
        let token = checkpoint.into_token();
        let c = subnet_actor_manager_facet::BottomUpCheckpoint::from_token(token)?;
        Ok(c)
    }

    async fn get_applied_top_down_nonce(&self, subnet_id: &SubnetID) -> Result<u64> {
        let route = subnet_id_to_evm_addresses(subnet_id)?;
        log::debug!("getting applied top down nonce for route: {route:?}");

        let evm_subnet_id = gateway_getter_facet::SubnetID {
            root: subnet_id.root_id(),
            route,
        };

        let gateway_contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );

        let (exists, nonce) = gateway_contract
            .get_applied_top_down_nonce(evm_subnet_id)
            .call()
            .await
            .map_err(|e| anyhow!("cannot get applied top down nonce due to: {e:}"))?;

        if !exists {
            Err(anyhow!("subnet {:?} does not exists", subnet_id))
        } else {
            Ok(nonce)
        }
    }

    async fn subnet_bottom_up_checkpoint_period(&self, subnet_id: &SubnetID) -> Result<ChainEpoch> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        Ok(contract.bottom_up_check_period().call().await? as ChainEpoch)
    }

    async fn prev_bottom_up_checkpoint_hash(
        &self,
        subnet_id: &SubnetID,
        epoch: ChainEpoch,
    ) -> Result<[u8; 32]> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let (exists, hash) = contract
            .bottom_up_checkpoint_hash_at_epoch(epoch as u64)
            .await?;
        if !exists {
            return if epoch == 0 {
                // first ever epoch, return empty bytes
                return Ok([0u8; 32]);
            } else {
                Err(anyhow!("checkpoint does not exists: {epoch:}"))
            };
        }
        Ok(hash)
    }

    async fn min_validators(&self, subnet_id: &SubnetID) -> Result<u64> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        Ok(contract.min_validators().call().await?)
    }
}

impl EthSubnetManager {
    pub fn new(
        gateway_addr: ethers::types::Address,
        registry_addr: ethers::types::Address,
        chain_id: u64,
        provider: Provider<Http>,
        keystore: Option<Arc<RwLock<PersistentKeyStore<EthKeyAddress>>>>,
    ) -> Self {
        Self {
            keystore,
            ipc_contract_info: IPCContractInfo {
                gateway_addr,
                registry_addr,
                chain_id,
                provider,
            },
        }
    }

    pub fn ensure_same_gateway(&self, gateway: &Address) -> Result<()> {
        let evm_gateway_addr = payload_to_evm_address(gateway.payload())?;
        if evm_gateway_addr != self.ipc_contract_info.gateway_addr {
            Err(anyhow!("Gateway address not matching with config"))
        } else {
            Ok(())
        }
    }

    pub fn keystore(&self) -> Result<Arc<RwLock<PersistentKeyStore<EthKeyAddress>>>> {
        self.keystore
            .clone()
            .ok_or(anyhow!("no evm keystore available"))
    }

    /// Get the ethers singer instance.
    /// We use filecoin addresses throughout our whole code-base
    /// and translate them to evm addresses when relevant.
    fn get_signer(&self, addr: &Address) -> Result<DefaultSignerMiddleware> {
        // convert to its underlying eth address
        let addr = payload_to_evm_address(addr.payload())?;
        let keystore = self.keystore()?;
        let keystore = keystore.read().unwrap();
        let private_key = keystore
            .get(&addr.into())?
            .ok_or_else(|| anyhow!("address {addr:} does not have private key in key store"))?;
        let wallet = LocalWallet::from_bytes(private_key.private_key())?
            .with_chain_id(self.ipc_contract_info.chain_id);

        Ok(SignerMiddleware::new(
            self.ipc_contract_info.provider.clone(),
            wallet,
        ))
    }

    pub fn from_subnet_with_wallet_store(
        subnet: &Subnet,
        keystore: Option<Arc<RwLock<PersistentKeyStore<EthKeyAddress>>>>,
    ) -> Result<Self> {
        let url = subnet.rpc_http().clone();
        let auth_token = subnet.auth_token();

        let SubnetConfig::Fevm(config) = &subnet.config;

        let provider = if auth_token.is_some() {
            Http::new_with_auth(url, Authorization::Bearer(auth_token.unwrap()))?
        } else {
            Http::new(url)
        };

        let mut provider = Provider::new(provider);
        // set polling interval for provider to fit fast child subnets block times.
        // TODO: We may want to make it dynamic so it adjusts depending on the type of network
        // so we don't have a too slow or too fast polling for the underlying block times.
        provider.set_interval(ETH_PROVIDER_POLLING_TIME);
        let gateway_address = payload_to_evm_address(config.gateway_addr.payload())?;
        let registry_address = payload_to_evm_address(config.registry_addr.payload())?;

        Ok(Self::new(
            gateway_address,
            registry_address,
            subnet.id.chain_id(),
            provider,
            keystore,
        ))
    }
}

#[async_trait]
impl BottomUpCheckpointRelayer for EthSubnetManager {
    async fn submit_checkpoint(
        &self,
        submitter: &Address,
        bundle: BottomUpCheckpointBundle,
    ) -> anyhow::Result<()> {
        let BottomUpCheckpointBundle {
            checkpoint,
            signatures,
            signatories,
            cross_msgs,
        } = bundle;

        let address = contract_address_from_subnet(&checkpoint.subnet_id)?;
        log::info!(
            "submit bottom up checkpoint: {checkpoint:?} in evm subnet contract: {address:}"
        );

        let signatures = signatures
            .into_iter()
            .map(ethers::types::Bytes::from)
            .collect::<Vec<_>>();
        let signatories = signatories
            .into_iter()
            .map(|addr| payload_to_evm_address(addr.payload()))
            .collect::<result::Result<Vec<_>, _>>()?;
        let cross_msgs = cross_msgs
            .into_iter()
            .map(subnet_actor_manager_facet::CrossMsg::try_from)
            .collect::<result::Result<Vec<_>, _>>()?;
        let checkpoint = subnet_actor_manager_facet::BottomUpCheckpoint::try_from(checkpoint)?;

        let signer = Arc::new(self.get_signer(submitter)?);
        let contract =
            subnet_actor_manager_facet::SubnetActorManagerFacet::new(address, signer.clone());
        let call = contract.submit_checkpoint(checkpoint, cross_msgs, signatories, signatures);
        call_with_premium_estimation(signer, call)
            .await?
            .send()
            .await?;

        Ok(())
    }

    async fn last_bottom_up_checkpoint_height(
        &self,
        subnet_id: &SubnetID,
    ) -> anyhow::Result<ChainEpoch> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let epoch = contract.last_bottom_up_checkpoint_height().call().await?;
        Ok(epoch as ChainEpoch)
    }

    async fn has_submitted_in_last_checkpoint_height(
        &self,
        subnet_id: &SubnetID,
        submitter: &Address,
    ) -> Result<bool> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let addr = payload_to_evm_address(submitter.payload())?;
        Ok(contract
            .has_submitted_in_last_bottom_up_checkpoint_height(addr)
            .call()
            .await?)
    }

    async fn checkpoint_period(&self, subnet_id: &SubnetID) -> anyhow::Result<ChainEpoch> {
        let address = contract_address_from_subnet(subnet_id)?;
        let contract = subnet_actor_getter_facet::SubnetActorGetterFacet::new(
            address,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );
        let epoch = contract.bottom_up_check_period().call().await?;
        Ok(epoch as ChainEpoch)
    }

    async fn checkpoint_bundle_at(
        &self,
        height: ChainEpoch,
    ) -> anyhow::Result<BottomUpCheckpointBundle> {
        let contract = gateway_getter_facet::GatewayGetterFacet::new(
            self.ipc_contract_info.gateway_addr,
            Arc::new(self.ipc_contract_info.provider.clone()),
        );

        let (checkpoint, _, signatories, signatures) =
            contract.get_signature_bundle(height as u64).call().await?;
        let cross_msgs = contract.bottom_up_messages(height as u64).call().await?;

        let checkpoint = BottomUpCheckpoint::try_from(checkpoint)?;
        let signatories = signatories
            .into_iter()
            .map(|s| ethers_address_to_fil_address(&s))
            .collect::<Result<Vec<_>, _>>()?;
        let signatures = signatures
            .into_iter()
            .map(|s| s.to_vec())
            .collect::<Vec<_>>();
        let cross_msgs = cross_msgs
            .into_iter()
            .map(CrossMsg::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BottomUpCheckpointBundle {
            checkpoint,
            signatures,
            signatories,
            cross_msgs,
        })
    }

    async fn current_epoch(&self) -> Result<ChainEpoch> {
        let epoch = self
            .ipc_contract_info
            .provider
            .get_block_number()
            .await?
            .as_u64();
        Ok(epoch as ChainEpoch)
    }
}

/// Receives an input `FunctionCall` and returns a new instance
/// after estimating an optimal `gas_premium` for the transaction
pub(crate) async fn call_with_premium_estimation<B, D, M>(
    signer: Arc<DefaultSignerMiddleware>,
    call: ethers_contract::FunctionCall<B, D, M>,
) -> Result<ethers_contract::FunctionCall<B, D, M>>
where
    B: std::borrow::Borrow<D>,
    M: ethers::abi::Detokenize,
{
    let (max_priority_fee_per_gas, _) = premium_estimation(signer).await?;
    Ok(call.gas_price(max_priority_fee_per_gas))
}

/// Returns an estimation of an optimal `gas_premium` and `gas_fee_cap`
/// for a transaction considering the average premium, base_fee and reward percentile from
/// past blocks
/// This is adaptation of ethers' `eip1559_default_estimator`:
/// https://github.com/gakonst/ethers-rs/blob/5dcd3b7e754174448f9a8cbfc0523896609629f9/ethers-core/src/utils/mod.rs#L476
async fn premium_estimation(
    signer: Arc<DefaultSignerMiddleware>,
) -> Result<(ethers::types::U256, ethers::types::U256)> {
    let base_fee_per_gas = signer
        .get_block(ethers::types::BlockNumber::Latest)
        .await?
        .ok_or_else(|| anyhow!("Latest block not found"))?
        .base_fee_per_gas
        .ok_or_else(|| anyhow!("EIP-1559 not activated"))?;

    let fee_history = signer
        .fee_history(
            ethers::utils::EIP1559_FEE_ESTIMATION_PAST_BLOCKS,
            ethers::types::BlockNumber::Latest,
            &[ethers::utils::EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE],
        )
        .await?;

    let max_priority_fee_per_gas = estimate_priority_fee(fee_history.reward); //overestimate?
    let potential_max_fee = base_fee_surged(base_fee_per_gas);
    let max_fee_per_gas = if max_priority_fee_per_gas > potential_max_fee {
        max_priority_fee_per_gas + potential_max_fee
    } else {
        potential_max_fee
    };

    Ok((max_priority_fee_per_gas, max_fee_per_gas))
}

/// Implementation borrowed from
/// https://github.com/gakonst/ethers-rs/blob/ethers-v2.0.8/ethers-core/src/utils/mod.rs#L582
/// Refer to the implementation for unit tests
fn base_fee_surged(base_fee_per_gas: U256) -> U256 {
    if base_fee_per_gas <= U256::from(40_000_000_000u64) {
        base_fee_per_gas * 2
    } else if base_fee_per_gas <= U256::from(100_000_000_000u64) {
        base_fee_per_gas * 16 / 10
    } else if base_fee_per_gas <= U256::from(200_000_000_000u64) {
        base_fee_per_gas * 14 / 10
    } else {
        base_fee_per_gas * 12 / 10
    }
}

/// Implementation borrowed from
/// https://github.com/gakonst/ethers-rs/blob/ethers-v2.0.8/ethers-core/src/utils/mod.rs#L536
/// Refer to the implementation for unit tests
fn estimate_priority_fee(rewards: Vec<Vec<U256>>) -> U256 {
    let mut rewards: Vec<U256> = rewards
        .iter()
        .map(|r| r[0])
        .filter(|r| *r > U256::zero())
        .collect();
    if rewards.is_empty() {
        return U256::zero();
    }
    if rewards.len() == 1 {
        return rewards[0];
    }
    // Sort the rewards as we will eventually take the median.
    rewards.sort();

    // A copy of the same vector is created for convenience to calculate percentage change
    // between subsequent fee values.
    let mut rewards_copy = rewards.clone();
    rewards_copy.rotate_left(1);

    let mut percentage_change: Vec<I256> = rewards
        .iter()
        .zip(rewards_copy.iter())
        .map(|(a, b)| {
            let a = I256::try_from(*a).expect("priority fee overflow");
            let b = I256::try_from(*b).expect("priority fee overflow");
            ((b - a) * 100) / a
        })
        .collect();
    percentage_change.pop();

    // Fetch the max of the percentage change, and that element's index.
    let max_change = percentage_change.iter().max().unwrap();
    let max_change_index = percentage_change
        .iter()
        .position(|&c| c == *max_change)
        .unwrap();

    // If we encountered a big change in fees at a certain position, then consider only
    // the values >= it.
    let values = if *max_change >= ethers::utils::EIP1559_FEE_ESTIMATION_THRESHOLD_MAX_CHANGE.into()
        && (max_change_index >= (rewards.len() / 2))
    {
        rewards[max_change_index..].to_vec()
    } else {
        rewards
    };

    // Return the median.
    values[values.len() / 2]
}

/// Get the block number from the transaction receipt
fn block_number_from_receipt(
    receipt: Option<ethers::types::TransactionReceipt>,
) -> Result<ChainEpoch> {
    match receipt {
        Some(r) => {
            let block_number = r
                .block_number
                .ok_or_else(|| anyhow!("cannot get block number"))?;
            Ok(block_number.as_u64() as ChainEpoch)
        }
        None => Err(anyhow!(
            "txn sent to network, but receipt cannot be obtained, please check scanner"
        )),
    }
}

/// Convert the ipc SubnetID type to an evm address. It extracts the last address from the Subnet id
/// children and turns it into evm address.
pub(crate) fn contract_address_from_subnet(subnet: &SubnetID) -> Result<ethers::types::Address> {
    let children = subnet.children();
    let ipc_addr = children
        .last()
        .ok_or_else(|| anyhow!("{subnet:} has no child"))?;

    payload_to_evm_address(ipc_addr.payload())
}

impl TryFrom<gateway_getter_facet::Subnet> for SubnetInfo {
    type Error = anyhow::Error;

    fn try_from(value: gateway_getter_facet::Subnet) -> Result<Self, Self::Error> {
        Ok(SubnetInfo {
            id: SubnetID::try_from(value.id)?,
            stake: eth_to_fil_amount(&value.stake)?,
            circ_supply: eth_to_fil_amount(&value.circ_supply)?,
            status: match value.status {
                1 => Status::Active,
                2 => Status::Inactive,
                3 => Status::Killed,
                _ => return Err(anyhow!("invalid status: {:}", value.status)),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::manager::evm::manager::contract_address_from_subnet;
    use fvm_shared::address::Address;
    use ipc_sdk::subnet_id::SubnetID;
    use std::str::FromStr;

    #[test]
    fn test_agent_subnet_to_evm_address() {
        let addr = Address::from_str("f410ffzyuupbyl2uiucmzr3lu3mtf3luyknthaz4xsrq").unwrap();
        let id = SubnetID::new(0, vec![addr]);

        let eth = contract_address_from_subnet(&id).unwrap();
        assert_eq!(
            format!("{eth:?}"),
            "0x2e714a3c385ea88a09998ed74db265dae9853667"
        );
    }
}
