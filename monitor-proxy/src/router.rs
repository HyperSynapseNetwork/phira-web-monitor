use crate::{handlers, middlewares, AppState};
use axum::{
    http::HeaderValue,
    middleware,
    routing::{get, post},
    Router,
};
use reqwest::{header, Method};
use tower_http::{cors::CorsLayer, services::ServeDir};

pub fn init_router(state: AppState) -> Router {
    let protected_routes = Router::new()
        .route("/auth/me", get(handlers::get_me_profile))
        .route("/ws/live", get(handlers::live_ws))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            middlewares::require_auth,
        ));
    let public_routes = Router::new()
        .route("/auth/login", post(handlers::login))
        .route("/chart/{id}", get(handlers::get_chart))
        .route("/rooms/info", get(handlers::get_room_list))
        .route("/rooms/info/{id}", get(handlers::get_room_by_id))
        .route("/rooms/user/{id}", get(handlers::get_room_of_user))
        .route("/rooms/listen", get(handlers::listen));

    // CORS configuration
    let cors = if state.config.debug {
        // Debug: mirror the request Origin header back
        // (Any + allow_credentials is forbidden by browsers and panics in tower-http)
        CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    } else {
        let origin: HeaderValue = state
            .config
            .allowed_origin
            .as_ref()
            .expect("--allowed-origin must be set")
            .parse()
            .expect("invalid --allowed-origin value");
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            .allow_credentials(true)
    };

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("../web/dist"))
}
