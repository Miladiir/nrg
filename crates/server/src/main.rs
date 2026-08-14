use tower_http::services::ServeDir;
use utoipa_swagger_ui::{Config, SwaggerUi};

#[tokio::main]
async fn main() {
    let swagger =
        SwaggerUi::new(nrg_api::SWAGGER_UI_PATH).config(Config::new([nrg_api::OPENAPI_JSON_PATH]));

    let app = nrg_api::router()
        .merge(swagger)
        .fallback_service(ServeDir::new("frontend"));

    let address = "0.0.0.0:8080";
    println!("Server running at http://{address}");
    println!("Swagger UI at http://{address}{}", nrg_api::SWAGGER_UI_PATH);

    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
