use actix_web::{
    get, post,
    web::{self, Data, Json},
    App, HttpResponse, HttpServer, Responder,
};
use jiff::{civil::Time, tz::TimeZone};
use tokio::sync::watch::{Receiver, Sender};

use crate::{
    frank::FrankStateLock,
    settings::{HeatAlarm, Settings, SettingsError, VibrationAlarm},
    SETTINGS_FILE,
};

pub async fn run(
    frank_state: FrankStateLock,
    settings_tx: Sender<Settings>,
    settings_rx: Receiver<Settings>,
) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(frank_state.clone()))
            .app_data(Data::new(settings_rx.clone()))
            .app_data(Data::new(settings_tx.clone()))
            .service(get_health)
            .service(get_state)
            .service(get_settings)
            .service(post_settings)
            .configure(cfg_settings_routes)
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}

#[get("/health")]
async fn get_health(frank_state: Data<FrankStateLock>) -> impl Responder {
    match frank_state.read().await.valid {
        true => HttpResponse::Ok().body("OK"),
        false => HttpResponse::InternalServerError().body("BAD"),
    }
}

#[get("/state")]
async fn get_state(frank_state: Data<FrankStateLock>) -> impl Responder {
    Json(frank_state.read().await.clone())
}

#[get("/settings")]
async fn get_settings(settings_rx: Data<Receiver<Settings>>) -> impl Responder {
    Json(settings_rx.borrow().clone())
}

#[post("/settings")]
async fn post_settings(
    settings_tx: Data<Sender<Settings>>,
    new_settings: Json<Settings>,
) -> impl Responder {
    let new_settings = new_settings.into_inner();
    if let Err(e) = new_settings.save(SETTINGS_FILE) {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    if settings_tx.send(new_settings).is_err() {
        return HttpResponse::InternalServerError().body("settings watch channel closed");
    }
    HttpResponse::Ok().body("OK")
}

#[get("/timezone")]
async fn get_timezone(settings_rx: Data<Receiver<Settings>>) -> impl Responder {
    let settings = settings_rx.borrow();
    let tz = settings.timezone.iana_name().map(|s| s.to_string());
    match tz {
        Some(s) => HttpResponse::Ok().body(s.clone()),
        None => HttpResponse::InternalServerError().body("Failed to get IANA name of timezone"),
    }
}

#[post("/timezone")]
async fn post_timezone(
    settings_rx: Data<Receiver<Settings>>,
    settings_tx: Data<Sender<Settings>>,
    new_tz: String,
) -> impl Responder {
    let mut settings = settings_rx.borrow().clone();

    match TimeZone::get(&new_tz) {
        Ok(tz) => settings.timezone = tz,
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    }

    if let Err(e) = settings.save(SETTINGS_FILE) {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    if settings_tx.send(settings).is_err() {
        return HttpResponse::InternalServerError().body("settings watch channel closed");
    }
    HttpResponse::Ok().body("OK")
}

#[get("/away_mode")]
async fn get_away_mode(settings_rx: Data<Receiver<Settings>>) -> impl Responder {
    let settings = settings_rx.borrow();
    HttpResponse::Ok().body(settings.away_mode.to_string())
}

#[post("/away_mode")]
async fn post_away_mode(
    settings_rx: Data<Receiver<Settings>>,
    settings_tx: Data<Sender<Settings>>,
    value: Json<bool>,
) -> impl Responder {
    let mut settings = settings_rx.borrow().clone();
    settings.away_mode = value.into_inner();

    if let Err(e) = settings.save(SETTINGS_FILE) {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    if settings_tx.send(settings).is_err() {
        return HttpResponse::InternalServerError().body("settings watch channel closed");
    }
    HttpResponse::Ok().body("OK")
}

#[get("/prime")]
async fn get_prime(settings_rx: Data<Receiver<Settings>>) -> impl Responder {
    let settings = settings_rx.borrow();
    Json(settings.prime)
}

#[post("/prime")]
async fn post_prime(
    settings_rx: Data<Receiver<Settings>>,
    settings_tx: Data<Sender<Settings>>,
    value: Json<Time>,
) -> impl Responder {
    let mut settings = settings_rx.borrow().clone();
    settings.prime = Some(value.into_inner());

    if let Err(e) = settings.save(SETTINGS_FILE) {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    if settings_tx.send(settings).is_err() {
        return HttpResponse::InternalServerError().body("settings watch channel closed");
    }
    HttpResponse::Ok().body("OK")
}

#[get("/led_brightness")]
async fn get_led_brightness(settings_rx: Data<Receiver<Settings>>) -> impl Responder {
    let settings = settings_rx.borrow();
    Json(settings.led_brightness)
}

#[post("/led_brightness")]
async fn post_led_brightness(
    settings_rx: Data<Receiver<Settings>>,
    settings_tx: Data<Sender<Settings>>,
    value: Json<u8>,
) -> impl Responder {
    let mut settings = settings_rx.borrow().clone();
    settings.led_brightness = Some(value.into_inner());

    if let Err(e) = settings.save(SETTINGS_FILE) {
        return HttpResponse::InternalServerError().body(e.to_string());
    }
    if settings_tx.send(settings).is_err() {
        return HttpResponse::InternalServerError().body("settings watch channel closed");
    }
    HttpResponse::Ok().body("OK")
}

macro_rules! define_settings_endpoints {
    (
        $(
            $field:ident : $typ:ty,
        )*
    ) => {
        paste::paste! {
            $(
                async fn [<get_both_ $field>](
                    settings_rx: Data<Receiver<Settings>>
                ) -> Result<impl Responder, SettingsError> {
                    let settings = settings_rx.borrow();
                    let res = settings.as_solo()?.$field.clone();
                    Ok(Json(res))
                }

                async fn [<get_left_ $field>](
                    settings_rx: Data<Receiver<Settings>>
                ) -> Result<impl Responder, SettingsError> {
                    let settings = settings_rx.borrow();
                    let res = settings.as_couples()?.0.$field.clone();
                    Ok(Json(res))
                }

                async fn [<get_right_ $field>](
                    settings_rx: Data<Receiver<Settings>>
                ) -> Result<impl Responder, SettingsError> {
                    let settings = settings_rx.borrow();
                    let res = settings.as_couples()?.1.$field.clone();
                    Ok(Json(res))
                }

                async fn [<post_both_ $field>](
                    settings_rx: Data<Receiver<Settings>>,
                    settings_tx: Data<Sender<Settings>>,
                    value: Json<$typ>,
                ) -> Result<impl Responder, SettingsError> {
                    let mut settings = settings_rx.borrow().clone();
                    settings.as_solo_mut()?.$field = value.into_inner();

                    if let Err(e) = settings.save(SETTINGS_FILE) {
                        return Ok(HttpResponse::InternalServerError().body(e.to_string()))
                    }
                    if settings_tx.send(settings).is_err() {
                        return Ok(HttpResponse::InternalServerError().body("settings watch channel closed"))
                    }
                    Ok(HttpResponse::Ok().body("OK"))
                }

                async fn [<post_left_ $field>](
                    settings_rx: Data<Receiver<Settings>>,
                    settings_tx: Data<Sender<Settings>>,
                    value: Json<$typ>,
                ) -> Result<impl Responder, SettingsError> {
                    let mut settings = settings_rx.borrow().clone();
                    settings.as_couples_mut()?.0.$field = value.into_inner();

                    if let Err(e) = settings.save(SETTINGS_FILE) {
                        return Ok(HttpResponse::InternalServerError().body(e.to_string()))
                    }
                    if settings_tx.send(settings).is_err() {
                        return Ok(HttpResponse::InternalServerError().body("settings watch channel closed"))
                    }
                    Ok(HttpResponse::Ok().body("OK"))
                }

                async fn [<post_right_ $field>](
                    settings_rx: Data<Receiver<Settings>>,
                    settings_tx: Data<Sender<Settings>>,
                    value: Json<$typ>,
                ) -> Result<impl Responder, SettingsError> {
                    let mut settings = settings_rx.borrow().clone();
                    settings.as_couples_mut()?.1.$field = value.into_inner();

                    if let Err(e) = settings.save(SETTINGS_FILE) {
                        return Ok(HttpResponse::InternalServerError().body(e.to_string()))
                    }
                    if settings_tx.send(settings).is_err() {
                        return Ok(HttpResponse::InternalServerError().body("settings watch channel closed"))
                    }
                    Ok(HttpResponse::Ok().body("OK"))
                }
            )*

            fn cfg_settings_routes(cfg: &mut web::ServiceConfig) {
                cfg
                $(
                    .route(concat!("/both/", stringify!($field)), web::get().to([<get_both_ $field>]))
                    .route(concat!("/left/", stringify!($field)), web::get().to([<get_left_ $field>]))
                    .route(concat!("/right/", stringify!($field)), web::get().to([<get_right_ $field>]))
                    .route(concat!("/both/", stringify!($field)), web::post().to([<post_both_ $field>]))
                    .route(concat!("/left/", stringify!($field)), web::post().to([<post_left_ $field>]))
                    .route(concat!("/right/", stringify!($field)), web::post().to([<post_right_ $field>]))
                )*;
            }
        }
    };
}

define_settings_endpoints!(
    temp_profile: Vec<i16>,
    wake: Time,
    sleep: Time,
    vibration: Option<VibrationAlarm>,
    heat: Option<HeatAlarm>,
);
