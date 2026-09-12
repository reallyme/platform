///Shorthand for `OwnedView<PlanLoadBalancerRequestView<'static>>`.
pub type OwnedPlanLoadBalancerRequestView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<PlanLoadBalancerResponseView<'static>>`.
pub type OwnedPlanLoadBalancerResponseView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ApplyLoadBalancerRequestView<'static>>`.
pub type OwnedApplyLoadBalancerRequestView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<ApplyLoadBalancerResponseView<'static>>`.
pub type OwnedApplyLoadBalancerResponseView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<DestroyLoadBalancerRequestView<'static>>`.
pub type OwnedDestroyLoadBalancerRequestView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<DestroyLoadBalancerResponseView<'static>>`.
pub type OwnedDestroyLoadBalancerResponseView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerResponseView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetLoadBalancerStatusRequestView<'static>>`.
pub type OwnedGetLoadBalancerStatusRequestView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusRequestView<
        'static,
    >,
>;
///Shorthand for `OwnedView<GetLoadBalancerStatusResponseView<'static>>`.
pub type OwnedGetLoadBalancerStatusResponseView = ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusResponseView<
        'static,
    >,
>;
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerResponse,
>
for crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerResponse,
>
for ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerResponse,
>
for crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerResponse,
>
for ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerResponse,
>
for crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerResponse,
>
for ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusResponse,
>
for crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusResponseView<
    '_,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self, codec)
    }
}
impl ::connectrpc::Encodable<
    crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusResponse,
>
for ::buffa::view::OwnedView<
    crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusResponseView<
        'static,
    >,
> {
    fn encode(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::buffa::bytes::Bytes, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body(self.reborrow(), codec)
    }
    /// An `OwnedView` still holds the buffer it was decoded from, so
    /// its large fields can be handed to the response body by
    /// reference count instead of copied. The bare view impl above
    /// cannot do this: it has borrows but no buffer to name.
    fn encode_segments(
        &self,
        codec: ::connectrpc::CodecFormat,
    ) -> ::std::result::Result<::connectrpc::EncodedBody, ::connectrpc::ConnectError> {
        ::connectrpc::__codegen::encode_view_body_segments(
            self.reborrow(),
            self.bytes(),
            codec,
        )
    }
}
/// Full service name for this service.
pub const LOAD_BALANCER_SERVICE_SERVICE_NAME: &str = "reallyme.hephaestus.v1.LoadBalancerService";
/// Static [`Spec`](::connectrpc::Spec) for the `PlanLoadBalancer` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LOAD_BALANCER_SERVICE_PLAN_LOAD_BALANCER_SPEC: ::connectrpc::Spec =
    ::connectrpc::Spec::server(
        "/reallyme.hephaestus.v1.LoadBalancerService/PlanLoadBalancer",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `ApplyLoadBalancer` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LOAD_BALANCER_SERVICE_APPLY_LOAD_BALANCER_SPEC: ::connectrpc::Spec =
    ::connectrpc::Spec::server(
        "/reallyme.hephaestus.v1.LoadBalancerService/ApplyLoadBalancer",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `DestroyLoadBalancer` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LOAD_BALANCER_SERVICE_DESTROY_LOAD_BALANCER_SPEC: ::connectrpc::Spec =
    ::connectrpc::Spec::server(
        "/reallyme.hephaestus.v1.LoadBalancerService/DestroyLoadBalancer",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// Static [`Spec`](::connectrpc::Spec) for the `GetLoadBalancerStatus` RPC, as seen by the server; the generated client passes it with [`origin`](::connectrpc::Spec::origin) `Client` (compare across sides with [`Spec::same_method`](::connectrpc::Spec::same_method)).
pub const LOAD_BALANCER_SERVICE_GET_LOAD_BALANCER_STATUS_SPEC: ::connectrpc::Spec =
    ::connectrpc::Spec::server(
        "/reallyme.hephaestus.v1.LoadBalancerService/GetLoadBalancerStatus",
        ::connectrpc::StreamType::Unary,
    )
    .with_idempotency_level(::connectrpc::IdempotencyLevel::Unknown);
/// LoadBalancerService is the provider-managed load-balancer orchestration
/// boundary. It models desired state independently from the OpenTofu or Ansible
/// adapter that will later execute it.
///
/// # Implementing handlers
///
/// Implement methods with plain `async fn`; the returned future satisfies
/// the `Send` bound automatically.
///
/// **Unary and server-streaming requests** arrive as
/// [`ServiceRequest<'_, Req>`](::connectrpc::ServiceRequest): a zero-copy
/// view of the request plus its body, valid for the duration of the call.
/// Fields are read directly (`request.name` is a `&str` into the decoded
/// buffer) and the borrow may be held across `.await` points. Anything
/// that must outlive the call — `tokio::spawn`, channels, server state,
/// or data captured by a returned response stream — takes owned data:
/// call `request.to_owned_message()` (or copy the specific fields)
/// first.
///
/// **Client-streaming and bidi requests** arrive as
/// [`InboundStream<Req>`](::connectrpc::InboundStream) — a
/// `ServiceStream` of [`StreamMessage`](::connectrpc::StreamMessage)s.
/// Each item owns its decoded buffer and is `Send + 'static`, so items
/// can be buffered or moved into spawned tasks; read fields zero-copy
/// through the generated accessor methods (`item.name()`) or `.view()`,
/// convert with `.to_owned_message()`, or yield an item back unchanged —
/// `StreamMessage<M>` implements `Encodable<M>`.
///
/// Request types resolved through `extern_path` (e.g. well-known types
/// from another crate) use the same wrappers; the crate that owns the
/// type must be generated with buffa ≥ 0.9.0 and views enabled so the
/// backing `HasMessageView` impl exists.
///
/// The `impl Encodable<Out>` return bound accepts the owned `Out`, the
/// generated `OutView<'_>` / `OwnedOutView`,
/// [`MaybeBorrowed`](::connectrpc::MaybeBorrowed), or
/// [`PreEncoded`](::connectrpc::PreEncoded) for handlers that encode a
/// non-`'static` view internally and pass the bytes across the handler
/// boundary. View bodies are not emitted for output types mapped via
/// `extern_path` (the impl would be an orphan); return owned for
/// WKT/extern outputs.
///
/// Server-streaming and bidi-streaming methods return
/// `ServiceStream<impl Encodable<Out> + Send + use<Self>>`. The
/// `use<Self>` precise-capturing clause excludes `&self`'s lifetime and
/// the request's lifetime (unary methods use `use<'a, Self>` and may
/// borrow from `&self`), so stream items must be `'static` and cannot
/// borrow from the request. To stream view-encoded data, encode each
/// item inside the stream body and yield
/// [`PreEncoded`](::connectrpc::PreEncoded) — see its `# Streaming
/// example` doc.
#[allow(clippy::type_complexity)]
pub trait LoadBalancerService: Send + Sync + 'static {
    /// Handle the PlanLoadBalancer RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn plan_load_balancer<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerResponse,
            > + Send
            + use<'a, Self>,
        >,
    > + Send;
    /// Handle the ApplyLoadBalancer RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn apply_load_balancer<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerResponse,
            > + Send
            + use<'a, Self>,
        >,
    > + Send;
    /// Handle the DestroyLoadBalancer RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn destroy_load_balancer<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerResponse,
            > + Send
            + use<'a, Self>,
        >,
    > + Send;
    /// Handle the GetLoadBalancerStatus RPC.
    ///
    /// `'a` lets the response body borrow from `&self` (e.g. server-resident state).
    ///
    /// `request` is borrowed from the request body and is valid for the
    /// duration of the call; message fields are read directly on it
    /// (zero-copy). The response cannot borrow from `request` — use
    /// `.to_owned_message()` (or copy the specific fields) for anything
    /// returned, stored, or moved into `tokio::spawn`.
    fn get_load_balancer_status<'a>(
        &'a self,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::ServiceRequest<
            '_,
            crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
        >,
    ) -> impl ::std::future::Future<
        Output = ::connectrpc::ServiceResult<
            impl ::connectrpc::Encodable<
                crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusResponse,
            > + Send
            + use<'a, Self>,
        >,
    > + Send;
}
/// Extension trait for registering a service implementation with a Router.
///
/// This trait is automatically implemented for all types that implement the service trait.
/// Prefer [`Router::add_service`](::connectrpc::Router::add_service) for
/// top-down registration; `register` remains available for compatibility
/// and cases where the service-first call shape is more convenient.
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
///
/// let service = Arc::new(MyServiceImpl);
/// let router = service.register(Router::new());
/// ```
pub trait LoadBalancerServiceExt: LoadBalancerService {
    /// Register this service implementation with a Router.
    ///
    /// Takes ownership of the `Arc<Self>` and returns a new Router with
    /// this service's methods registered.
    fn register(self: ::std::sync::Arc<Self>, router: ::connectrpc::Router)
    -> ::connectrpc::Router;
}
impl<S: LoadBalancerService> LoadBalancerServiceExt for S {
    fn register(
        self: ::std::sync::Arc<Self>,
        router: ::connectrpc::Router,
    ) -> ::connectrpc::Router {
        router
            .route_view(
                LOAD_BALANCER_SERVICE_SERVICE_NAME,
                "PlanLoadBalancer",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.plan_load_balancer(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LOAD_BALANCER_SERVICE_PLAN_LOAD_BALANCER_SPEC)
            .route_view(
                LOAD_BALANCER_SERVICE_SERVICE_NAME,
                "ApplyLoadBalancer",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.apply_load_balancer(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LOAD_BALANCER_SERVICE_APPLY_LOAD_BALANCER_SPEC)
            .route_view(
                LOAD_BALANCER_SERVICE_SERVICE_NAME,
                "DestroyLoadBalancer",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.destroy_load_balancer(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LOAD_BALANCER_SERVICE_DESTROY_LOAD_BALANCER_SPEC)
            .route_view(
                LOAD_BALANCER_SERVICE_SERVICE_NAME,
                "GetLoadBalancerStatus",
                {
                    let svc = ::std::sync::Arc::clone(&self);
                    ::connectrpc::view_handler_fn(move |
                        ctx,
                        req: ::buffa::view::OwnedView<
                            crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusRequestView<
                                'static,
                            >,
                        >,
                        format|
                    {
                        let svc = ::std::sync::Arc::clone(&svc);
                        async move {
                            let sreq = ::connectrpc::ServiceRequest::<
                                crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
                            >::from_parts(req.reborrow(), req.bytes());
                            svc.get_load_balancer_status(ctx, sreq)
                                .await?
                                .encode::<
                                    crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusResponse,
                                >(format)
                        }
                    })
                },
            )
            .with_spec(LOAD_BALANCER_SERVICE_GET_LOAD_BALANCER_STATUS_SPEC)
    }
}
/// Type-inference marker used by [`Router::add_service`](::connectrpc::Router::add_service).
#[doc(hidden)]
pub struct LoadBalancerServiceRegisterMarker;
impl<S: LoadBalancerService> ::connectrpc::ServiceRegister<LoadBalancerServiceRegisterMarker>
    for ::std::sync::Arc<S>
{
    fn register_service(self, router: ::connectrpc::Router) -> ::connectrpc::Router {
        <S as LoadBalancerServiceExt>::register(self, router)
    }
}
/// Monomorphic dispatcher for `LoadBalancerService`.
///
/// Unlike `.register(Router)` which type-erases each method into an `Arc<dyn ErasedHandler>` stored in a `HashMap`, this struct dispatches via a compile-time `match` on method name: no vtable, no hash lookup.
///
/// # Example
///
/// ```rust,ignore
/// use connectrpc::ConnectRpcService;
///
/// let server = LoadBalancerServiceServer::new(MyImpl);
/// let service = ConnectRpcService::new(server);
/// // hand `service` to axum/hyper as a fallback_service
/// ```
pub struct LoadBalancerServiceServer<T> {
    inner: ::std::sync::Arc<T>,
}
impl<T: LoadBalancerService> LoadBalancerServiceServer<T> {
    /// Wrap a service implementation in a monomorphic dispatcher.
    pub fn new(service: T) -> Self {
        Self {
            inner: ::std::sync::Arc::new(service),
        }
    }
    /// Wrap an already-`Arc`'d service implementation.
    pub fn from_arc(inner: ::std::sync::Arc<T>) -> Self {
        Self { inner }
    }
}
impl<T> Clone for LoadBalancerServiceServer<T> {
    fn clone(&self) -> Self {
        Self {
            inner: ::std::sync::Arc::clone(&self.inner),
        }
    }
}
impl<T: LoadBalancerService> ::connectrpc::Dispatcher for LoadBalancerServiceServer<T> {
    #[inline]
    fn lookup(&self, path: &str) -> Option<::connectrpc::dispatcher::codegen::MethodDescriptor> {
        let method = path.strip_prefix("reallyme.hephaestus.v1.LoadBalancerService/")?;
        match method {
            "PlanLoadBalancer" => Some(
                ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                    .with_spec(LOAD_BALANCER_SERVICE_PLAN_LOAD_BALANCER_SPEC),
            ),
            "ApplyLoadBalancer" => Some(
                ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                    .with_spec(LOAD_BALANCER_SERVICE_APPLY_LOAD_BALANCER_SPEC),
            ),
            "DestroyLoadBalancer" => Some(
                ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                    .with_spec(LOAD_BALANCER_SERVICE_DESTROY_LOAD_BALANCER_SPEC),
            ),
            "GetLoadBalancerStatus" => Some(
                ::connectrpc::dispatcher::codegen::MethodDescriptor::unary(false)
                    .with_spec(LOAD_BALANCER_SERVICE_GET_LOAD_BALANCER_STATUS_SPEC),
            ),
            _ => None,
        }
    }
    fn call_unary(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::connectrpc::Payload,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("reallyme.hephaestus.v1.LoadBalancerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            "PlanLoadBalancer" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
                    >::from_parts(&req, &body);
                    svc.plan_load_balancer(ctx, req)
                        .await?
                        .encode::<
                            crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerResponse,
                        >(format)
                })
            }
            "ApplyLoadBalancer" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
                    >::from_parts(&req, &body);
                    svc.apply_load_balancer(ctx, req)
                        .await?
                        .encode::<
                            crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerResponse,
                        >(format)
                })
            }
            "DestroyLoadBalancer" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
                    >::from_parts(&req, &body);
                    svc.destroy_load_balancer(ctx, req)
                        .await?
                        .encode::<
                            crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerResponse,
                        >(format)
                })
            }
            "GetLoadBalancerStatus" => {
                let svc = ::std::sync::Arc::clone(&self.inner);
                Box::pin(async move {
                    let body = ::connectrpc::dispatcher::codegen::request_proto_bytes::<
                        crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
                    >(request.encoded()?, format)?;
                    let req: crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusRequestView<
                        '_,
                    > = ::connectrpc::dispatcher::codegen::decode_borrowed_request_view(
                        &body,
                        ctx.decode_options(),
                    )?;
                    let req = ::connectrpc::ServiceRequest::<
                        crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
                    >::from_parts(&req, &body);
                    svc.get_load_balancer_status(ctx, req)
                        .await?
                        .encode::<
                            crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusResponse,
                        >(format)
                })
            }
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_server_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        request: ::buffa::bytes::Bytes,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("reallyme.hephaestus.v1.LoadBalancerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &request, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
    fn call_client_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::UnaryResult {
        let Some(method) = path.strip_prefix("reallyme.hephaestus.v1.LoadBalancerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_unary(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_unary(path),
        }
    }
    fn call_bidi_streaming(
        &self,
        path: &str,
        ctx: ::connectrpc::RequestContext,
        requests: ::connectrpc::dispatcher::codegen::RequestStream,
        format: ::connectrpc::CodecFormat,
    ) -> ::connectrpc::dispatcher::codegen::StreamingResult {
        let Some(method) = path.strip_prefix("reallyme.hephaestus.v1.LoadBalancerService/") else {
            return ::connectrpc::dispatcher::codegen::unimplemented_streaming(path);
        };
        let _ = (&ctx, &requests, &format);
        match method {
            _ => ::connectrpc::dispatcher::codegen::unimplemented_streaming(path),
        }
    }
}
/// Client for this service.
///
/// Generic over `T: ClientTransport`. For **gRPC** (HTTP/2), use
/// `Http2Connection` — it has honest `poll_ready` and composes with
/// `tower::balance` for multi-connection load balancing. For **Connect
/// over HTTP/1.1** (or unknown protocol), use `HttpClient`.
///
/// # Example (gRPC / HTTP/2)
///
/// ```rust,ignore
/// use connectrpc::client::{Http2Connection, ClientConfig};
/// use connectrpc::Protocol;
///
/// let uri: http::Uri = "http://localhost:8080".parse()?;
/// let conn = Http2Connection::connect_plaintext(uri.clone()).await?.shared(1024);
/// let config = ClientConfig::new(uri).with_protocol(Protocol::Grpc);
///
/// let client = LoadBalancerServiceClient::new(conn, config);
/// let response = client.plan_load_balancer(request).await?;
/// ```
///
/// # Example (Connect / HTTP/1.1 or ALPN)
///
/// ```rust,ignore
/// use connectrpc::client::{HttpClient, ClientConfig};
///
/// let http = HttpClient::plaintext();  // cleartext http:// only
/// let config = ClientConfig::new("http://localhost:8080".parse()?);
///
/// let client = LoadBalancerServiceClient::new(http, config);
/// let response = client.plan_load_balancer(request).await?;
/// ```
///
/// # Working with the response
///
/// Unary calls return [`UnaryResponse<OwnedView<FooView>>`](::connectrpc::client::UnaryResponse).
/// [`view()`](::connectrpc::client::UnaryResponse::view) borrows the response
/// message, so field access is zero-copy:
///
/// ```rust,ignore
/// let resp = client.plan_load_balancer(request).await?;
/// let name: &str = resp.view().name;  // borrow into the response buffer
/// ```
///
/// If you need the owned struct (e.g. to store or pass by value), use
/// [`into_owned()`](::connectrpc::client::UnaryResponse::into_owned):
///
/// ```rust,ignore
/// let owned = client.plan_load_balancer(request).await?.into_owned();
/// ```
///
/// [`into_view()`](::connectrpc::client::UnaryResponse::into_view) keeps the
/// zero-copy decoded body (an `OwnedView`) without copying; field access on it
/// goes through `.reborrow()`. Streaming responses yield one
/// [`StreamMessage`](::connectrpc::StreamMessage) per received message from
/// `.message().await` — read fields zero-copy through the generated accessor
/// methods (`msg.name()`) or `.view()`, or convert with `.to_owned_message()`.
#[derive(Clone)]
pub struct LoadBalancerServiceClient<T> {
    transport: T,
    config: ::connectrpc::client::ClientConfig,
}
impl<T> LoadBalancerServiceClient<T>
where
    T: ::connectrpc::client::ClientTransport,
    <T::ResponseBody as ::connectrpc::http_body::Body>::Error: ::std::fmt::Display,
{
    /// Create a new client with the given transport and configuration.
    pub fn new(transport: T, config: ::connectrpc::client::ClientConfig) -> Self {
        Self { transport, config }
    }
    /// Get the client configuration.
    pub fn config(&self) -> &::connectrpc::client::ClientConfig {
        &self.config
    }
    /// Get a mutable reference to the client configuration.
    pub fn config_mut(&mut self) -> &mut ::connectrpc::client::ClientConfig {
        &mut self.config
    }
    /// Call the PlanLoadBalancer RPC. Sends a request to /reallyme.hephaestus.v1.LoadBalancerService/PlanLoadBalancer.
    pub async fn plan_load_balancer(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        self.plan_load_balancer_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the PlanLoadBalancer RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn plan_load_balancer_with_options(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::PlanLoadBalancerRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::PlanLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        ::connectrpc::client::call_unary(
            &self.transport,
            &self.config,
            LOAD_BALANCER_SERVICE_PLAN_LOAD_BALANCER_SPEC
                .with_origin(::connectrpc::SpecOrigin::Client),
            request,
            options,
        )
        .await
    }
    /// Call the ApplyLoadBalancer RPC. Sends a request to /reallyme.hephaestus.v1.LoadBalancerService/ApplyLoadBalancer.
    pub async fn apply_load_balancer(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        self.apply_load_balancer_with_options(request, ::connectrpc::client::CallOptions::default())
            .await
    }
    /// Call the ApplyLoadBalancer RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn apply_load_balancer_with_options(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::ApplyLoadBalancerRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::ApplyLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        ::connectrpc::client::call_unary(
            &self.transport,
            &self.config,
            LOAD_BALANCER_SERVICE_APPLY_LOAD_BALANCER_SPEC
                .with_origin(::connectrpc::SpecOrigin::Client),
            request,
            options,
        )
        .await
    }
    /// Call the DestroyLoadBalancer RPC. Sends a request to /reallyme.hephaestus.v1.LoadBalancerService/DestroyLoadBalancer.
    pub async fn destroy_load_balancer(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        self.destroy_load_balancer_with_options(
            request,
            ::connectrpc::client::CallOptions::default(),
        )
        .await
    }
    /// Call the DestroyLoadBalancer RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn destroy_load_balancer_with_options(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::DestroyLoadBalancerRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::DestroyLoadBalancerResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        ::connectrpc::client::call_unary(
            &self.transport,
            &self.config,
            LOAD_BALANCER_SERVICE_DESTROY_LOAD_BALANCER_SPEC
                .with_origin(::connectrpc::SpecOrigin::Client),
            request,
            options,
        )
        .await
    }
    /// Call the GetLoadBalancerStatus RPC. Sends a request to /reallyme.hephaestus.v1.LoadBalancerService/GetLoadBalancerStatus.
    pub async fn get_load_balancer_status(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        self.get_load_balancer_status_with_options(
            request,
            ::connectrpc::client::CallOptions::default(),
        )
        .await
    }
    /// Call the GetLoadBalancerStatus RPC with explicit per-call options. Options override [`ClientConfig`](::connectrpc::client::ClientConfig) defaults.
    pub async fn get_load_balancer_status_with_options(
        &self,
        request: crate::generated::proto::reallyme::hephaestus::v1::GetLoadBalancerStatusRequest,
        options: ::connectrpc::client::CallOptions,
    ) -> Result<
        ::connectrpc::client::UnaryResponse<
            ::buffa::view::OwnedView<
                crate::generated::proto::reallyme::hephaestus::v1::__buffa::view::GetLoadBalancerStatusResponseView<
                    'static,
                >,
            >,
        >,
        ::connectrpc::ConnectError,
    >{
        ::connectrpc::client::call_unary(
            &self.transport,
            &self.config,
            LOAD_BALANCER_SERVICE_GET_LOAD_BALANCER_STATUS_SPEC
                .with_origin(::connectrpc::SpecOrigin::Client),
            request,
            options,
        )
        .await
    }
}
