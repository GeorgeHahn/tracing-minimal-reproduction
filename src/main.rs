use std::{sync::mpsc, thread};
use tracing::*;
use tracing_core::Dispatch;
use tracing_subscriber::{EnvFilter, reload, prelude::*};

pub fn create_subscriber(
    filter: &'_ str,
) -> (
        Dispatch,
        Box<dyn Fn(EnvFilter) -> Result<(), tracing_subscriber::reload::Error> + Send + Sync>,
    )
{
    let fmt = tracing_subscriber::fmt::layer();
    let (filter, handle) = reload::Layer::new(EnvFilter::new(filter));
    let subscriber = tracing_subscriber::registry()
        .with(fmt)
        .with(filter);
    let dispatch = Dispatch::new(subscriber);
    let change_level = Box::new(move |filter| handle.clone().reload(filter));
    (dispatch, change_level)
}

fn main() {
    let filter = "warn";
    let (dispatch, reload) = create_subscriber(filter);
    let d2 = dispatch.clone();
    tracing::dispatcher::set_global_default(d2).expect("set default dispatch");

    // Nothing here is printed (filter is at warn)
    {
        let s = info_span!("thread1-before");
        let _g = s.enter();
        trace!("trace");
        debug!("debug");
        info!("info");
    }

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // Reload to `trace` level filter
        (reload)(EnvFilter::new("trace")).expect("reload filter");
        tx.send(()).expect("send complete signal");

        // All of these should emit. They do.
        {
            let s = info_span!("thread2-after");
            let _g = s.enter();
            trace!("trace");
            debug!("debug");
            info!("info");
            warn!("warn");
            error!("error");
        }
    });
    rx.recv().expect("receive complete signal");

    // All of these should emit. They do.
    {
        let s = info_span!("thread1-after");
        let _g = s.enter();
        trace!("trace");
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");
    }
}
