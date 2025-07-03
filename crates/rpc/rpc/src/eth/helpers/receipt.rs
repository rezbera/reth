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
use reth_rpc_eth_types::receipt::ReceiptBuilderProvider;
use reth_storage_api::{BlockReader, ReceiptProvider, TransactionsProvider};

use crate::EthApi;

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
