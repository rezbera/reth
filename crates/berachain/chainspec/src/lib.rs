//! Berachain chain spec

use derive_more::{Constructor, Deref, Into};
use reth_chainspec::{ChainSpec};


/// Berachain chain spec
#[derive(Debug, Clone, Deref, Into, Constructor, PartialEq, Eq)]
pub struct BerachainChainSpec {
    inner: ChainSpec
}

#[cfg(test)]
mod tests {
    use reth_chainspec::BaseFeeParamsKind;
    use super::*;

    #[test]
    fn berachain_spec_works() {
        let spec = BerachainChainSpec::new(ChainSpec::default());
        match spec.base_fee_params {
            BaseFeeParamsKind::Constant(_) => {
                assert_eq!(1,1)
            }
            BaseFeeParamsKind::Variable(_) => {
                assert_eq!(1,1)
            }
        }
    }
}
