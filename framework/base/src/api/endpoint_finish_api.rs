use super::{ErrorApi, HandleTypeInfo};

pub trait EndpointFinishApi: HandleTypeInfo + ErrorApi {
    type EndpointFinishApiImpl: EndpointFinishApiImpl
        + HandleTypeInfo<
            ManagedBufferHandle = Self::ManagedBufferHandle,
            BigIntHandle = Self::BigIntHandle,
            BigFloatHandle = Self::BigFloatHandle,
            EllipticCurveHandle = Self::EllipticCurveHandle,
        >;

    fn finish_api_impl() -> Self::EndpointFinishApiImpl;
}

/// Interface to only be used by code generated by the macros.
/// The smart contract code doesn't have access to these methods directly.
pub trait EndpointFinishApiImpl: HandleTypeInfo {
    fn finish_slice_u8(&self, slice: &[u8]);

    fn finish_big_int_raw(&self, handle: Self::BigIntHandle);

    fn finish_big_uint_raw(&self, handle: Self::BigIntHandle);

    fn finish_managed_buffer_raw(&self, handle: Self::ManagedBufferHandle);

    fn finish_u64(&self, value: u64);

    fn finish_i64(&self, value: i64);
}
