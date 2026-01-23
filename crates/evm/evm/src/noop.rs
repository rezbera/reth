//! Helpers for testing.

use crate::{ConfigureEvm, EvmEnvFor, EvmLimitParams};
use reth_primitives_traits::{BlockTy, HeaderTy, SealedBlock, SealedHeader};

/// A no-op EVM config that panics on any call. Used as a typesystem hack to satisfy
/// [`ConfigureEvm`] bounds.
#[derive(Debug, Clone)]
pub struct NoopEvmConfig<Inner>(core::marker::PhantomData<Inner>);

impl<Inner> Default for NoopEvmConfig<Inner> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Inner> NoopEvmConfig<Inner> {
    /// Create a new instance of the no-op EVM config.
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }

    fn inner(&self) -> &Inner {
        unimplemented!("NoopEvmConfig should never be called")
    }
}

impl<Inner> ConfigureEvm for NoopEvmConfig<Inner>
where
    Inner: ConfigureEvm,
{
    type Primitives = Inner::Primitives;
    type Error = Inner::Error;
    type NextBlockEnvCtx = Inner::NextBlockEnvCtx;
    type BlockExecutorFactory = Inner::BlockExecutorFactory;
    type BlockAssembler = Inner::BlockAssembler;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        self.inner().block_executor_factory()
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        self.inner().block_assembler()
    }

    fn evm_limit_params_at_timestamp(&self, timestamp: u64) -> EvmLimitParams {
        self.inner().evm_limit_params_at_timestamp(timestamp)
    }

    fn evm_env_without_limits(
        &self,
        header: &HeaderTy<Self::Primitives>,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        self.inner().evm_env_without_limits(header)
    }

    fn next_evm_env_without_limits(
        &self,
        parent: &HeaderTy<Self::Primitives>,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<EvmEnvFor<Self>, Self::Error> {
        self.inner().next_evm_env_without_limits(parent, attributes)
    }

    fn context_for_block<'a>(
        &self,
        block: &'a SealedBlock<BlockTy<Self::Primitives>>,
    ) -> Result<crate::ExecutionCtxFor<'a, Self>, Self::Error> {
        self.inner().context_for_block(block)
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader<HeaderTy<Self::Primitives>>,
        attributes: Self::NextBlockEnvCtx,
    ) -> Result<crate::ExecutionCtxFor<'_, Self>, Self::Error> {
        self.inner().context_for_next_block(parent, attributes)
    }
}
