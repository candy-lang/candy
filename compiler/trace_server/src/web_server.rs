use std::sync::{Arc, RwLock};

use actix_web::{
    get, http::StatusCode, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use tracing::debug;

use crate::{storage::TraceStorage, trace::TraceId};

async fn run(storage: Arc<RwLock<TraceStorage>>) {
    let server = HttpServer::new(move || {
        App::new()
            .app_data(storage.clone())
            .service(trace_with_id)
            .default_service(web::route().to(default_handler))
    });

    let Ok(server) = server.bind("localhost:5000") else {
        panic!("Can't bind to localhost:5000.");
    };
    debug!("Server running on http://localhost:5000");
    server.run().await.expect("Server crashed.");
}

// #[get("/")]
// async fn index(blog: web::Data<Blog>) -> impl Responder {
//     HttpResponse::Ok().cached().html("fds").await)
// }

async fn default_handler(req: HttpRequest) -> impl Responder {
    HttpResponse::Ok()
        .status(StatusCode::NOT_FOUND)
        .html("default_handler")
}

#[get("/trace/{id}")]
async fn trace_with_id(req: HttpRequest, path: web::Path<(String,)>) -> impl Responder {
    let (id,) = path.into_inner();

    let Ok(id) = id.parse() else {
        return HttpResponse::Error().status(StatusCode::NOT_FOUND);
    };
    let id = TraceId::from_usize(id);

    let storage = req.app_data::<web::Data<TraceStorage>>().unwrap();
    let trace = storage.get(id);

    HttpResponse::Ok().json(format!("{{\"id\": {}}}", id))
}
