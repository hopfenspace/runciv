use std::future::{ready, Ready};

use actix_toolbox::tb_middleware::actix_session::SessionExt;
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use futures::future::LocalBoxFuture;

use crate::server::handler::ApiError;

pub(crate) struct AuthenticationRequired;

impl<S, B> Transform<S, ServiceRequest> for AuthenticationRequired
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = AuthenticationRequiredMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthenticationRequiredMiddleware { service }))
    }
}

pub(crate) struct AuthenticationRequiredMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthenticationRequiredMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session = req.get_session();

        let logged_in = session
            .get("logged_in")
            .map(|logged_in_maybe| logged_in_maybe.map_or(false, |v| v));

        let next = self.service.call(req);
        Box::pin(async move {
            if !logged_in.map_err(ApiError::SessionGet)? {
                return Err(ApiError::Unauthenticated.into());
            }

            next.await
        })
    }
}
