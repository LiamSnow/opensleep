use std::time::Duration;

use jiff::{civil::Time, tz::TimeZone, SignedDuration, Timestamp, ToSpan, Unit, Zoned};
use log::error;
use thiserror::Error;
use tokio::{sync::{
    mpsc,
    watch::{Receiver, Ref},
}, time::interval};

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
}

pub async fn run(frank_tx: mpsc::Sender<FrankCommand>, mut cfg_rx: Receiver<Settings>) -> Result<(), SchedulerError> {
    loop {
        let cfg = cfg_rx.borrow_and_update();

        if let Some(bri) = cfg.led_brightness {
            let res = frank_tx.send(FrankCommand::SetSettings(Box::new(FrankSettings {
                version: 1,
                gain_right: 400,
                gain_left: 400,
                led_brightness_perc: bri,
            }))).await;
            if let Err(e) = res {
                error!("Frank channel error {e}");
            }
        }

        if !cfg.away_mode {
            let tz = &cfg.timezone.clone();
            let mut schedule = make_schedule(cfg)?;
            schedule.sort_by_key(|(z, _)| z.clone());

            for (mut next, cmd) in schedule {
                loop {
                    let now = Timestamp::now().to_zoned(tz.clone());
                    if next > now {
                        let dur = Duration::from_secs(now.until(&next)?.total(Unit::Second)? as u64);
                        tokio::select! {
                            _ = tokio::time::sleep(dur) => {
                                let res = frank_tx.send(cmd.clone()).await;
                                if let Err(e) = res {
                                    error!("Frank channel error {e}");
                                }
                                next = next.checked_add(1.day())?;
                            }
                            _ = cfg_rx.changed() => {
                                break;
                            }
                        }
                    } else {
                        next = next.checked_add(1.day())?;
                    }
                }
            }
        } else {
            // borrow checker being hella annoying
            // here so this is my fix
            let mut interval = interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                if cfg.has_changed() {
                    break
                }
            }
        }
    }
}

fn make_schedule(cfg: Ref<'_, Settings>) -> Result<Vec<(Zoned, FrankCommand)>, SchedulerError> {
    let mut res = Vec::new();

    let now = Timestamp::now().to_zoned(cfg.timezone.clone());

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
        let set = Box::new((vib.clone(), vib_dt.time(), tz.clone()));
        res.push((vib_dt, FrankCommand::SetAlarm(tar.clone(), set)));
    }

    if let Some(heat) = &cfg.heat {
        let heat_start_dt = wake_dt.clone();
        wake_dt = wake_dt.checked_sub(SignedDuration::from_secs(heat.offset.into()))?;
        res.push((
            heat_start_dt,
            FrankCommand::SetTemp(tar.clone(), heat.temp, heat.offset),
        ));
    }

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
