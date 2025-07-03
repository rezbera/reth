//! RPC receipt response builder, extends a layer one receipt with layer two data.

use super::EthResult;
use alloy_consensus::{transaction::TransactionMeta, ReceiptEnvelope, TxReceipt};
use alloy_eips::eip7840::BlobParams;
use alloy_primitives::{Address, TxKind};
use alloy_rpc_types_eth::{Log, ReceiptWithBloom, TransactionReceipt};
use reth_ethereum_primitives::{Receipt, TransactionSigned, TxType};
use reth_primitives_traits::SignedTransaction;


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
pub trait ReceiptBuilderProvider<Tx, R> {
    /// The receipt builder type.
    type Builder: RpcReceiptBuilder<Tx, R>;

    /// Get the receipt builder instance.
    fn receipt_builder(&self) -> &Self::Builder;
}

/// RPC receipt response building strategy for different transaction and receipt types.
/// 
/// This trait abstracts RPC receipt response building to allow custom transaction envelopes
/// to integrate with the RPC layer while reusing the core receipt building logic.
/// 
/// Note: This is different from alloy's `ReceiptBuilder` which operates at the EVM execution
/// layer. This trait operates at the RPC response layer.
pub trait RpcReceiptBuilder<Tx, R> {
    /// The envelope type for the receipt response.
    type ReceiptEnvelope;
    
    /// Build a transaction receipt from transaction and receipt data.
    fn build_transaction_receipt(
        &self,
        tx: &Tx,
        meta: TransactionMeta,
        receipt: &R,
        all_receipts: &[R],
        blob_params: Option<BlobParams>,
    ) -> EthResult<TransactionReceipt<Self::ReceiptEnvelope>>;
}

/// Ethereum receipt builder strategy.
/// 
/// Uses the standard Ethereum receipt building logic with EthereumTxEnvelope.
#[derive(Debug, Default, Clone)]
pub struct EthReceiptBuilderStrategy;

impl RpcReceiptBuilder<TransactionSigned, Receipt> for EthReceiptBuilderStrategy {
    type ReceiptEnvelope = ReceiptEnvelope<Log>;

    fn build_transaction_receipt(
        &self,
        tx: &TransactionSigned,
        meta: TransactionMeta,
        receipt: &Receipt,
        all_receipts: &[Receipt],
        blob_params: Option<BlobParams>,
    ) -> EthResult<TransactionReceipt<Self::ReceiptEnvelope>> {
        Ok(EthReceiptBuilder::new(tx, meta, receipt, all_receipts, blob_params)?.build())
    }
}

/// Builds a [`TransactionReceipt`] obtaining the inner receipt envelope from the given closure.
/// 
/// This is the core receipt building function that handles all the common logic for
/// constructing receipts. The envelope building strategy is provided as a closure
/// to allow different chains to customize the envelope type.
pub fn build_receipt<R, T, E>(
    transaction: &T,
    meta: TransactionMeta,
    receipt: &R,
    all_receipts: &[R],
    blob_params: Option<BlobParams>,
    build_envelope: impl FnOnce(ReceiptWithBloom<alloy_consensus::Receipt<Log>>) -> E,
) -> EthResult<TransactionReceipt<E>>
where
    R: TxReceipt<Log = alloy_primitives::Log>,
    T: SignedTransaction,
{
    // Note: we assume this transaction is valid, because it's mined (or part of pending block)
    // and we don't need to check for pre EIP-2
    let from = transaction.recover_signer_unchecked()?;

    // get the previous transaction cumulative gas used
    let gas_used = if meta.index == 0 {
        receipt.cumulative_gas_used()
    } else {
        let prev_tx_idx = (meta.index - 1) as usize;
        all_receipts
            .get(prev_tx_idx)
            .map(|prev_receipt| receipt.cumulative_gas_used() - prev_receipt.cumulative_gas_used())
            .unwrap_or_default()
    };

    let blob_gas_used = transaction.blob_gas_used();
    // Blob gas price should only be present if the transaction is a blob transaction
    let blob_gas_price =
        blob_gas_used.and_then(|_| Some(blob_params?.calc_blob_fee(meta.excess_blob_gas?)));

    let logs_bloom = receipt.bloom();

    // get number of logs in the block
    let mut num_logs = 0;
    for prev_receipt in all_receipts.iter().take(meta.index as usize) {
        num_logs += prev_receipt.logs().len();
    }

    let logs: Vec<Log> = receipt
        .logs()
        .iter()
        .enumerate()
        .map(|(tx_log_idx, log)| Log {
            inner: log.clone(),
            block_hash: Some(meta.block_hash),
            block_number: Some(meta.block_number),
            block_timestamp: Some(meta.timestamp),
            transaction_hash: Some(meta.tx_hash),
            transaction_index: Some(meta.index),
            log_index: Some((num_logs + tx_log_idx) as u64),
            removed: false,
        })
        .collect();

    let rpc_receipt = alloy_rpc_types_eth::Receipt {
        status: receipt.status_or_post_state(),
        cumulative_gas_used: receipt.cumulative_gas_used(),
        logs,
    };

    let (contract_address, to) = match transaction.kind() {
        TxKind::Create => (Some(from.create(transaction.nonce())), None),
        TxKind::Call(addr) => (None, Some(Address(*addr))),
    };

    Ok(TransactionReceipt {
        inner: build_envelope(ReceiptWithBloom { receipt: rpc_receipt, logs_bloom }),
        transaction_hash: meta.tx_hash,
        transaction_index: Some(meta.index),
        block_hash: Some(meta.block_hash),
        block_number: Some(meta.block_number),
        from,
        to,
        gas_used,
        contract_address,
        effective_gas_price: transaction.effective_gas_price(meta.base_fee),
        // EIP-4844 fields
        blob_gas_price,
        blob_gas_used,
    })
}

/// Receipt response builder.
#[derive(Debug)]
pub struct EthReceiptBuilder {
    /// The base response body, contains L1 fields.
    pub base: TransactionReceipt,
}

impl EthReceiptBuilder {
    /// Returns a new builder with the base response body (L1 fields) set.
    ///
    /// Note: This requires _all_ block receipts because we need to calculate the gas used by the
    /// transaction.
    pub fn new(
        transaction: &TransactionSigned,
        meta: TransactionMeta,
        receipt: &Receipt,
        all_receipts: &[Receipt],
        blob_params: Option<BlobParams>,
    ) -> EthResult<Self> {
        let base = build_receipt(
            transaction,
            meta,
            receipt,
            all_receipts,
            blob_params,
            |receipt_with_bloom| match receipt.tx_type {
                TxType::Legacy => ReceiptEnvelope::Legacy(receipt_with_bloom),
                TxType::Eip2930 => ReceiptEnvelope::Eip2930(receipt_with_bloom),
                TxType::Eip1559 => ReceiptEnvelope::Eip1559(receipt_with_bloom),
                TxType::Eip4844 => ReceiptEnvelope::Eip4844(receipt_with_bloom),
                TxType::Eip7702 => ReceiptEnvelope::Eip7702(receipt_with_bloom),
            },
        )?;

        Ok(Self { base })
    }

    /// Builds a receipt response from the base response body, and any set additional fields.
    pub fn build(self) -> TransactionReceipt {
        self.base
    }
}
