//! Builds an RPC receipt response w.r.t. data layout of network.
//! 
//! This module provides a generic receipt building strategy pattern that allows
//! custom transaction envelopes to integrate with the RPC layer while reusing
//! core receipt building logic. The implementation is generic over transaction
//! and receipt types, delegating type-specific logic to RpcReceiptBuilder implementations.

use alloy_consensus::transaction::TransactionMeta;
use reth_chainspec::{ChainSpecProvider, EthChainSpec};
use reth_ethereum_primitives::{Receipt, TransactionSigned};
use reth_rpc_eth_api::{helpers::LoadReceipt, FromEthApiError, RpcNodeCoreExt, RpcReceipt};
use reth_rpc_eth_types::{EthApiError, receipt::{RpcReceiptBuilder, EthReceiptBuilderStrategy}};
use reth_storage_api::{BlockReader, ReceiptProvider, TransactionsProvider};

use crate::EthApi;

/// # Generic Receipt Building Pattern
/// 
/// This module demonstrates how to integrate custom transaction envelopes with reth's RPC layer:
/// 
/// ## For Custom Chains (e.g., Berachain):
/// 
/// ```rust,ignore
/// // 1. Define your custom receipt builder
/// pub struct BerachainReceiptBuilderStrategy;
/// 
/// impl RpcReceiptBuilder<BerachainTxEnvelope, Receipt> for BerachainReceiptBuilderStrategy {
///     type ReceiptEnvelope = BerachainReceiptEnvelope; // Your custom envelope type
///     
///     fn build_transaction_receipt(
///         &self,
///         tx: &BerachainTxEnvelope,
///         meta: TransactionMeta,
///         receipt: &Receipt,
///         all_receipts: &[Receipt],
///         blob_params: Option<BlobParams>,
///     ) -> EthResult<TransactionReceipt<Self::ReceiptEnvelope>> {
///         match tx {
///             BerachainTxEnvelope::Ethereum(eth_tx) => {
///                 // Delegate to Ethereum builder for standard transactions
///                 let eth_builder = EthReceiptBuilderStrategy;
///                 let eth_result = eth_builder.build_transaction_receipt(
///                     eth_tx, meta, receipt, all_receipts, blob_params
///                 )?;
///                 // Convert result to your envelope type
///                 Ok(convert_eth_to_berachain_receipt(eth_result))
///             }
///             BerachainTxEnvelope::SystemReward(pol_tx) => {
///                 // Custom logic for POL/SystemReward transactions
///                 build_system_reward_receipt(pol_tx, meta, receipt, all_receipts)
///             }
///         }
///     }
/// }
/// 
/// // 2. Implement ReceiptBuilderProvider for your API type
/// impl<Provider, Pool, Network, EvmConfig> ReceiptBuilderProvider<BerachainTxEnvelope, Receipt> 
/// for BerachainEthApi<Provider, Pool, Network, EvmConfig> 
/// {
///     type Builder = BerachainReceiptBuilderStrategy;
///     
///     fn receipt_builder(&self) -> &Self::Builder {
///         &BERACHAIN_BUILDER // Your static instance
///     }
/// }
/// 
/// // 3. The generic LoadReceipt implementation will automatically work!
/// // No need to reimplement LoadReceipt - it works with any transaction type
/// // that has a ReceiptBuilderProvider implementation.
/// ```
/// 
/// ## Key Benefits:
/// - **Reuse**: Core receipt logic is shared across all chains
/// - **Flexibility**: Each chain can customize transaction-specific receipt building
/// - **Type Safety**: Full compile-time verification of receipt envelope types
/// - **Backward Compatibility**: Existing Ethereum code continues to work unchanged

/// Trait for providing a receipt builder strategy.
pub(super) trait ReceiptBuilderProvider<Tx, R> {
    /// The receipt builder type.
    type Builder: RpcReceiptBuilder<Tx, R>;
    
    /// Get the receipt builder instance.
    fn receipt_builder(&self) -> &Self::Builder;
}

/// Default implementation for EthApi with TransactionSigned.
impl<Provider, Pool, Network, EvmConfig> ReceiptBuilderProvider<TransactionSigned, Receipt> 
for EthApi<Provider, Pool, Network, EvmConfig> 
where
    Provider: BlockReader,
{
    type Builder = EthReceiptBuilderStrategy;
    
    fn receipt_builder(&self) -> &Self::Builder {
        // Use a static instance since EthReceiptBuilderStrategy is stateless
        static BUILDER: EthReceiptBuilderStrategy = EthReceiptBuilderStrategy;
        &BUILDER
    }
}

// Type aliases to simplify complex associated type constraints
type ApiReceiptBuilder<Api, Tx, R> = <Api as ReceiptBuilderProvider<Tx, R>>::Builder;
type BuilderEnvelope<Api, Tx, R> = <ApiReceiptBuilder<Api, Tx, R> as RpcReceiptBuilder<Tx, R>>::ReceiptEnvelope;

// Generic implementation that works with any transaction/receipt type
impl<Provider, Pool, Network, EvmConfig, Tx, R> LoadReceipt for EthApi<Provider, Pool, Network, EvmConfig>
where
    Tx: reth_primitives_traits::SignedTransaction,
    R: alloy_consensus::TxReceipt<Log = alloy_primitives::Log>,
    Self: RpcNodeCoreExt<
        Provider: TransactionsProvider<Transaction = Tx>
                      + ReceiptProvider<Receipt = R>,
    > + ReceiptBuilderProvider<Tx, R>,
    Provider: BlockReader + ChainSpecProvider,
    // The key constraint: builder output must convert to API receipt type
    RpcReceipt<Self::NetworkTypes>: From<alloy_rpc_types_eth::TransactionReceipt<BuilderEnvelope<Self, Tx, R>>>,
{
    async fn build_transaction_receipt(
        &self,
        tx: Tx,
        meta: TransactionMeta,
        receipt: R,
    ) -> Result<RpcReceipt<Self::NetworkTypes>, Self::Error> {
        let hash = meta.block_hash;
        // get all receipts for the block
        let all_receipts = self
            .cache()
            .get_receipts(hash)
            .await
            .map_err(Self::Error::from_eth_err)?
            .ok_or(EthApiError::HeaderNotFound(hash.into()))?;
        let blob_params = self.provider().chain_spec().blob_params_at_timestamp(meta.timestamp);

        // Use the receipt builder strategy
        let builder = self.receipt_builder();
        let receipt_result = builder.build_transaction_receipt(
            &tx, meta, &receipt, &all_receipts, blob_params
        )?;
        
        // Convert from builder output to API receipt type
        Ok(receipt_result.into())
    }
}
