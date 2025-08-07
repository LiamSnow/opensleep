use std::time::Duration;

use jiff::{civil::Time, tz::TimeZone, SignedDuration, Timestamp, ToSpan, Unit, Zoned};
use log::{error, info};
use thiserror::Error;
use tokio::{
    sync::{
        mpsc,
        watch::{error::RecvError, Receiver, Ref},
    },
    time::sleep,
};

use crate::{
    frank::{
        command::{FrankCommand, SideTarget},
        state::FrankSettings,
    },
    settings::{BySideSettings, Settings, SideSettings},
};

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("jiff error: `{0}`")]
    Jiff(#[from] jiff::Error),
    #[error("watch channel revc error: `{0}`")]
    Watch(#[from] RecvError),
}

/// This function tries to never crash, unless there is Jiff error, in which case we want to crash
/// (either its a core issue that needs to be fixed or a configuration issue)
pub async fn run(
    frank_tx: mpsc::Sender<FrankCommand>,
    mut cfg_rx: Receiver<Settings>,
) -> Result<(), SchedulerError> {
    loop {
        let handle = {
            let cfg = cfg_rx.borrow_and_update();

            // set settings
            if let Some(bri) = cfg.led_brightness {
                let res = frank_tx
                    .send(FrankCommand::SetSettings(Box::new(FrankSettings {
                        version: 1,
                        gain_right: 400,
                        gain_left: 400,
                        led_brightness_perc: bri,
                    })))
                    .await;
                if let Err(e) = res {
                    error!("[Scheduler] Frank channel error {e}");
                }
            }

            // make schedule and run it
            if !cfg.away_mode {
                let tz = cfg.timezone.clone();
                let mut schedule = make_schedule(cfg)?;
                schedule.sort_by_key(|(z, _)| z.clone());

                info!(
                    "[Scheduler] New schedule has {} events: {:#?}",
                    schedule.len(),
                    schedule
                );

                Some(tokio::spawn(task(frank_tx.clone(), schedule, tz)).abort_handle())
            } else {
                None
            }
        };

        // wait until next change
        cfg_rx.changed().await?;
        handle.map(|h| h.abort());
        info!("[Scheduler] Settings have changed! Restarting...");
    }
}

/// run schedule daily
pub async fn task(
    frank_tx: mpsc::Sender<FrankCommand>,
    mut schedule: Vec<(Zoned, FrankCommand)>,
    tz: TimeZone,
) -> Result<(), SchedulerError> {
    loop {
        for (next, cmd) in &mut schedule {
            let now = Timestamp::now().to_zoned(tz.clone());
            if now < *next {
                let dur = Duration::from_secs(now.until(&*next)?.total(Unit::Second)? as u64);
                info!("[Scheduler] Waiting {dur:#?}");
                sleep(dur).await;
            }

            let res = frank_tx.send(cmd.clone()).await;
            if let Err(e) = res {
                error!("[Scheduler] Frank channel error {e}");
            }

            *next = next.checked_add(1.day())?;
        }
    }
}

fn make_schedule(cfg: Ref<'_, Settings>) -> Result<Vec<(Zoned, FrankCommand)>, SchedulerError> {
    let mut res = Vec::new();

    let now = Timestamp::now().to_zoned(cfg.timezone.clone());

    info!("[Scheduler] Making schedule at {now}");

    if let Some(prime_time) = cfg.prime {
        let mut prime_dt = now.with().time(prime_time).build()?;
        if now > prime_dt {
            prime_dt = prime_dt.tomorrow()?;
        }
        res.push((prime_dt, FrankCommand::Prime));
    }

    match &cfg.by_side {
        BySideSettings::Couples { left, right } => {
            schedule_side(&mut res, left, SideTarget::Left, &now, &cfg.timezone)?;
            schedule_side(&mut res, right, SideTarget::Right, &now, &cfg.timezone)?;
        }
        BySideSettings::Solo { both } => {
            schedule_side(&mut res, both, SideTarget::Both, &now, &cfg.timezone)?;
        }
    }

    Ok(res)
}

fn schedule_side(
    res: &mut Vec<(Zoned, FrankCommand)>,
    cfg: &SideSettings,
    tar: SideTarget,
    now: &Zoned,
    tz: &TimeZone,
) -> Result<(), SchedulerError> {
    let (sleep_dt, mut wake_dt) = calc_sleep_wake_dts(now, cfg.sleep, cfg.wake)?;

    if let Some(vib) = &cfg.vibration {
        let vib_dt = wake_dt.checked_sub(SignedDuration::from_secs(vib.offset.into()))?;
        let vib_settings = Box::new((vib.clone(), vib_dt.time(), tz.clone()));
        // let Frank know about the alarm ahead of time
        let set_vib_dt = vib_dt.checked_sub(SignedDuration::from_mins(3))?;
        res.push((set_vib_dt, FrankCommand::SetAlarm(tar.clone(), vib_settings)));
    }

    if let Some(heat) = &cfg.heat {
        let heat_start_dt = wake_dt.clone();
        wake_dt = wake_dt.checked_sub(SignedDuration::from_secs(heat.offset.into()))?;
        res.push((
            heat_start_dt,
            FrankCommand::SetTemp(tar.clone(), heat.temp, heat.offset),
        ));
    }

    info!("[Scheduler] Result for {tar:?}: sleep at {sleep_dt}, wake at {wake_dt}");

    calc_profile(res, tar, &cfg.temp_profile, sleep_dt, wake_dt)?;

    Ok(())
}

fn calc_sleep_wake_dts(
    now: &Zoned,
    sleep_time: Time,
    wake_time: Time,
) -> Result<(Zoned, Zoned), SchedulerError> {
    let mut sleep = now.with().time(sleep_time).build()?;
    let mut wake = now.with().time(wake_time).build()?;

    // idk why this messes with my head so much

    // 20:00 -> 7:00
    if sleep > wake {
        // @ 3:00 -> 20:00 yesterday, 7:00 today
        if now < wake {
            sleep = sleep.yesterday()?;
        }
        // @ 10:00 = 20:00 today, 7:00 tomorrow
        // @ 19:00 = 20:00 today, 7:00 tomorrow
        // @ 21:00 = 20:00 today, 7:00 tomorrow
        else {
            wake = wake.tomorrow()?;
        }
    }
    // # 1:00 -> 9:00
    //  @ 3:00 = 1:00 today, 9:00 today
    //  @ 10:00 = 1:00 tomorrow, 9:00 tomorrow
    //  @ 19:00 = 1:00 tomorrow, 9:00 tomorrow
    //
    // # 12:00 -> 16:00
    //  @ 3:00 = 12:00 today, 16:00 today
    //  @ 13:00 = 12:00 today, 16:00 today
    //  @ 17:00 = 12:00 tomorrow, 16:00 tomorrow
    else if now > wake {
        sleep = sleep.tomorrow()?;
        wake = wake.tomorrow()?;
    }

    Ok((sleep, wake))
}

// TODO ideally this will change the temperature
// every ~1 min for a gradual tempature change
fn calc_profile(
    res: &mut Vec<(Zoned, FrankCommand)>,
    tar: SideTarget,
    prof: &Vec<i16>,
    sleep_dt: Zoned,
    wake_dt: Zoned,
) -> Result<(), SchedulerError> {
    let sleep_period = sleep_dt.until(&wake_dt)?.total(Unit::Second)? as i64;
    let step_len = SignedDuration::from_secs(sleep_period / prof.len() as i64);
    let step_len_secs = step_len.as_secs() as u16;

    info!("[Scheduler] Result for {tar:?}: sleep period {sleep_period} seconds with each step {step_len_secs} seconds");

    for (i, temp) in prof.iter().enumerate() {
        let dt = sleep_dt.checked_add(step_len * i as i32)?;
        res.push((dt, FrankCommand::SetTemp(tar.clone(), *temp, step_len_secs)));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use jiff::{
        civil::{time, Time},
        tz::TimeZone,
        Timestamp, Zoned,
    };

    use crate::frank::command::{FrankCommand, SideTarget};

    use super::{calc_profile, calc_sleep_wake_dts};

    fn today_at(hour: i8, minute: i8) -> Zoned {
        Timestamp::now()
            .to_zoned(TimeZone::system())
            .with()
            .time(time(hour, minute, 0, 0))
            .build()
            .unwrap()
    }

    fn tomorrow_at(hour: i8, minute: i8) -> Zoned {
        today_at(hour, minute).tomorrow().unwrap()
    }

    fn yesterday_at(hour: i8, minute: i8) -> Zoned {
        today_at(hour, minute).yesterday().unwrap()
    }

    fn ctime(hour: i8, minute: i8) -> Time {
        time(hour, minute, 0, 0)
    }

    #[test]
    fn test_normal_sleep_schedule() {
        let sleep_time = ctime(22, 0);
        let wake_time = ctime(7, 0);

        let now = today_at(3, 0);
        assert_eq!(
            calc_sleep_wake_dts(&now, sleep_time, wake_time).unwrap(),
            (yesterday_at(22, 0), today_at(7, 0))
        );

        let now = today_at(10, 0);
        assert_eq!(
            calc_sleep_wake_dts(&now, sleep_time, wake_time).unwrap(),
            (today_at(22, 0), tomorrow_at(7, 0))
        );
    }

    #[test]
    fn test_bad_sleep_schedule() {
        let sleep_time = ctime(1, 0);
        let wake_time = ctime(9, 0);

        let now = today_at(3, 0);
        assert_eq!(
            calc_sleep_wake_dts(&now, sleep_time, wake_time).unwrap(),
            (today_at(1, 0), today_at(9, 0))
        );

        let now = today_at(10, 0);
        assert_eq!(
            calc_sleep_wake_dts(&now, sleep_time, wake_time).unwrap(),
            (tomorrow_at(1, 0), tomorrow_at(9, 0))
        );
    }

    #[test]
    fn test_profile() {
        let sleep_dt = today_at(23, 0);
        let wake_dt = tomorrow_at(8, 0);
        let prof = vec![-10, 0, 10];

        let tar = SideTarget::Both;
        let mut actual = Vec::new();
        calc_profile(&mut actual, tar.clone(), &prof, sleep_dt, wake_dt).unwrap();
        let step_len_secs = 3 * 3600 as u16;

        let expected = vec![
            (
                today_at(23, 0),
                FrankCommand::SetTemp(tar.clone(), -10, step_len_secs),
            ),
            (
                tomorrow_at(2, 0),
                FrankCommand::SetTemp(tar.clone(), 0, step_len_secs),
            ),
            (
                tomorrow_at(5, 0),
                FrankCommand::SetTemp(tar, 10, step_len_secs),
            ),
        ];

        assert_eq!(actual, expected);
    }
}
