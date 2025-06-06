use crate::{
    size::InMemorySize,
    transaction::signed::{RecoveryError, SignedTransaction},
};
use alloc::vec::Vec;
use alloy_consensus::{transaction::SignerRecoverable, Transaction};
use alloy_eips::{
    eip2718::{Eip2718Error, Eip2718Result, IsTyped2718},
    eip2930::AccessList,
    eip7702::SignedAuthorization,
    Decodable2718, Encodable2718, Typed2718,
};
use alloy_primitives::{ChainId, TxHash};
use alloy_rlp::{BufMut, Decodable, Encodable, Result as RlpResult};
use revm_primitives::{Address, Bytes, TxKind, B256, U256};

macro_rules! delegate {
    ($self:expr => $tx:ident.$method:ident($($arg:expr),*)) => {
        match $self {
            Self::BuiltIn($tx) => $tx.$method($($arg),*),
            Self::Other($tx) => $tx.$method($($arg),*),
        }
    };
}

/// A [`SignedTransaction`] implementation that combines two different transaction types.
///
/// This is intended to be used to extend existing presets, for example the ethereum or optstack
/// transaction types.
///
/// Note: The other transaction type variants must not overlap with the builtin one, transaction
/// types must be unique.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ExtendedTxEnvelope<BuiltIn, Other> {
    /// The builtin transaction type.
    BuiltIn(BuiltIn),
    /// The other transaction type.
    Other(Other),
}

impl<B, T> Transaction for ExtendedTxEnvelope<B, T>
where
    B: Transaction,
    T: Transaction,
{
    fn chain_id(&self) -> Option<ChainId> {
        delegate!(self => tx.chain_id())
    }

    fn nonce(&self) -> u64 {
        delegate!(self => tx.nonce())
    }

    fn gas_limit(&self) -> u64 {
        delegate!(self => tx.gas_limit())
    }

    fn gas_price(&self) -> Option<u128> {
        delegate!(self => tx.gas_price())
    }

    fn max_fee_per_gas(&self) -> u128 {
        delegate!(self => tx.max_fee_per_gas())
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        delegate!(self => tx.max_priority_fee_per_gas())
    }

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        delegate!(self => tx.max_fee_per_blob_gas())
    }

    fn priority_fee_or_price(&self) -> u128 {
        delegate!(self => tx.priority_fee_or_price())
    }

    fn effective_gas_price(&self, base_fee: Option<u64>) -> u128 {
        delegate!(self => tx.effective_gas_price(base_fee))
    }

    fn is_dynamic_fee(&self) -> bool {
        delegate!(self => tx.is_dynamic_fee())
    }

    fn kind(&self) -> TxKind {
        delegate!(self => tx.kind())
    }

    fn is_create(&self) -> bool {
        match self {
            Self::BuiltIn(tx) => tx.is_create(),
            Self::Other(_tx) => false,
        }
    }

    fn value(&self) -> U256 {
        delegate!(self => tx.value())
    }

    fn input(&self) -> &Bytes {
        delegate!(self => tx.input())
    }

    fn access_list(&self) -> Option<&AccessList> {
        delegate!(self => tx.access_list())
    }

    fn blob_versioned_hashes(&self) -> Option<&[B256]> {
        delegate!(self => tx.blob_versioned_hashes())
    }

    fn authorization_list(&self) -> Option<&[SignedAuthorization]> {
        delegate!(self => tx.authorization_list())
    }
}

impl<B, T> IsTyped2718 for ExtendedTxEnvelope<B, T>
where
    B: IsTyped2718,
    T: IsTyped2718,
{
    fn is_type(type_id: u8) -> bool {
        B::is_type(type_id) || T::is_type(type_id)
    }
}

impl<B, T> InMemorySize for ExtendedTxEnvelope<B, T>
where
    B: InMemorySize,
    T: InMemorySize,
{
    fn size(&self) -> usize {
        delegate!(self => tx.size())
    }
}

impl<B, T> SignerRecoverable for ExtendedTxEnvelope<B, T>
where
    B: SignedTransaction + IsTyped2718,
    T: SignedTransaction,
{
    fn recover_signer(&self) -> Result<Address, RecoveryError> {
        delegate!(self => tx.recover_signer())
    }

    fn recover_signer_unchecked(&self) -> Result<Address, RecoveryError> {
        delegate!(self => tx.recover_signer_unchecked())
    }
}

impl<B, T> SignedTransaction for ExtendedTxEnvelope<B, T>
where
    B: SignedTransaction + IsTyped2718,
    T: SignedTransaction,
{
    fn tx_hash(&self) -> &TxHash {
        match self {
            Self::BuiltIn(tx) => tx.tx_hash(),
            Self::Other(tx) => tx.tx_hash(),
        }
    }

    fn recover_signer_unchecked_with_buf(
        &self,
        buf: &mut Vec<u8>,
    ) -> Result<Address, RecoveryError> {
        delegate!(self => tx.recover_signer_unchecked_with_buf(buf))
    }
}

impl<B, T> Typed2718 for ExtendedTxEnvelope<B, T>
where
    B: Typed2718,
    T: Typed2718,
{
    fn ty(&self) -> u8 {
        match self {
            Self::BuiltIn(tx) => tx.ty(),
            Self::Other(tx) => tx.ty(),
        }
    }
}

impl<B, T> Decodable2718 for ExtendedTxEnvelope<B, T>
where
    B: Decodable2718 + IsTyped2718,
    T: Decodable2718,
{
    fn typed_decode(ty: u8, buf: &mut &[u8]) -> Eip2718Result<Self> {
        if B::is_type(ty) {
            let envelope = B::typed_decode(ty, buf)?;
            Ok(Self::BuiltIn(envelope))
        } else {
            let other = T::typed_decode(ty, buf)?;
            Ok(Self::Other(other))
        }
    }
    fn fallback_decode(buf: &mut &[u8]) -> Eip2718Result<Self> {
        if buf.is_empty() {
            return Err(Eip2718Error::RlpError(alloy_rlp::Error::InputTooShort));
        }
        B::fallback_decode(buf).map(Self::BuiltIn)
    }
}

impl<B, T> Encodable2718 for ExtendedTxEnvelope<B, T>
where
    B: Encodable2718,
    T: Encodable2718,
{
    fn encode_2718_len(&self) -> usize {
        match self {
            Self::BuiltIn(envelope) => envelope.encode_2718_len(),
            Self::Other(tx) => tx.encode_2718_len(),
        }
    }

    fn encode_2718(&self, out: &mut dyn BufMut) {
        match self {
            Self::BuiltIn(envelope) => envelope.encode_2718(out),
            Self::Other(tx) => tx.encode_2718(out),
        }
    }
}

impl<B, T> Encodable for ExtendedTxEnvelope<B, T>
where
    B: Encodable,
    T: Encodable,
{
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            Self::BuiltIn(envelope) => envelope.encode(out),
            Self::Other(tx) => tx.encode(out),
        }
    }

    fn length(&self) -> usize {
        match self {
            Self::BuiltIn(envelope) => envelope.length(),
            Self::Other(tx) => tx.length(),
        }
    }
}

impl<B, T> Decodable for ExtendedTxEnvelope<B, T>
where
    B: Decodable,
    T: Decodable,
{
    fn decode(buf: &mut &[u8]) -> RlpResult<Self> {
        let original = *buf;

        match B::decode(buf) {
            Ok(tx) => Ok(Self::BuiltIn(tx)),
            Err(_) => {
                *buf = original;
                T::decode(buf).map(Self::Other)
            }
        }
    }
}

#[cfg(feature = "op")]
mod op {
    use crate::ExtendedTxEnvelope;
    use alloy_consensus::error::ValueError;
    use alloy_primitives::{Signature, B256};
    use op_alloy_consensus::{OpPooledTransaction, OpTxEnvelope};

    impl<Tx> TryFrom<ExtendedTxEnvelope<OpTxEnvelope, Tx>>
        for ExtendedTxEnvelope<OpPooledTransaction, Tx>
    {
        type Error = OpTxEnvelope;

        fn try_from(value: ExtendedTxEnvelope<OpTxEnvelope, Tx>) -> Result<Self, Self::Error> {
            match value {
                ExtendedTxEnvelope::BuiltIn(tx) => {
                    let converted_tx: OpPooledTransaction =
                        tx.clone().try_into().map_err(|_| tx)?;
                    Ok(Self::BuiltIn(converted_tx))
                }
                ExtendedTxEnvelope::Other(tx) => Ok(Self::Other(tx)),
            }
        }
    }

    impl<Tx> From<OpPooledTransaction> for ExtendedTxEnvelope<OpTxEnvelope, Tx> {
        fn from(tx: OpPooledTransaction) -> Self {
            Self::BuiltIn(tx.into())
        }
    }

    impl<Tx> TryFrom<ExtendedTxEnvelope<OpTxEnvelope, Tx>> for OpPooledTransaction {
        type Error = ValueError<OpTxEnvelope>;

        fn try_from(_tx: ExtendedTxEnvelope<OpTxEnvelope, Tx>) -> Result<Self, Self::Error> {
            match _tx {
                ExtendedTxEnvelope::BuiltIn(inner) => inner.try_into(),
                ExtendedTxEnvelope::Other(_tx) => Err(ValueError::new(
                    OpTxEnvelope::Legacy(alloy_consensus::Signed::new_unchecked(
                        alloy_consensus::TxLegacy::default(),
                        Signature::decode_rlp_vrs(&mut &[0u8; 65][..], |_| Ok(false)).unwrap(),
                        B256::default(),
                    )),
                    "Cannot convert custom transaction to OpPooledTransaction",
                )),
            }
        }
    }
}

#[cfg(feature = "serde-bincode-compat")]
mod serde_bincode_compat {
    use super::*;
    use crate::serde_bincode_compat::SerdeBincodeCompat;

    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    #[derive(Debug)]
    pub enum ExtendedTxEnvelopeRepr<'a, B: SerdeBincodeCompat, T: SerdeBincodeCompat> {
        BuiltIn(B::BincodeRepr<'a>),
        Other(T::BincodeRepr<'a>),
    }

    impl<B, T> SerdeBincodeCompat for ExtendedTxEnvelope<B, T>
    where
        B: SerdeBincodeCompat + core::fmt::Debug,
        T: SerdeBincodeCompat + core::fmt::Debug,
    {
        type BincodeRepr<'a> = ExtendedTxEnvelopeRepr<'a, B, T>;

        fn as_repr(&self) -> Self::BincodeRepr<'_> {
            match self {
                Self::BuiltIn(tx) => ExtendedTxEnvelopeRepr::BuiltIn(tx.as_repr()),
                Self::Other(tx) => ExtendedTxEnvelopeRepr::Other(tx.as_repr()),
            }
        }

        fn from_repr(repr: Self::BincodeRepr<'_>) -> Self {
            match repr {
                ExtendedTxEnvelopeRepr::BuiltIn(tx_repr) => Self::BuiltIn(B::from_repr(tx_repr)),
                ExtendedTxEnvelopeRepr::Other(tx_repr) => Self::Other(T::from_repr(tx_repr)),
            }
        }
    }
}

#[cfg(feature = "reth-codec")]
use alloy_primitives::bytes::Buf;

#[cfg(feature = "reth-codec")]
impl<B, T> reth_codecs::Compact for ExtendedTxEnvelope<B, T>
where
    B: Transaction + IsTyped2718 + reth_codecs::Compact,
    T: Transaction + reth_codecs::Compact,
{
    fn to_compact<Buf>(&self, buf: &mut Buf) -> usize
    where
        Buf: alloy_rlp::bytes::BufMut + AsMut<[u8]>,
    {
        buf.put_u8(self.ty());
        match self {
            Self::BuiltIn(tx) => tx.to_compact(buf),
            Self::Other(tx) => tx.to_compact(buf),
        }
    }

    fn from_compact(mut buf: &[u8], len: usize) -> (Self, &[u8]) {
        let type_byte = buf.get_u8();

        if <B as IsTyped2718>::is_type(type_byte) {
            let (tx, remaining) = B::from_compact(buf, len);
            return (Self::BuiltIn(tx), remaining);
        }

        let (tx, remaining) = T::from_compact(buf, len);
        (Self::Other(tx), remaining)
    }
}
