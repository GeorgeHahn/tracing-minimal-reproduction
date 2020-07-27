use opentelemetry::api::Provider;
use opentelemetry::sdk;
use std::{sync::{Arc, mpsc}, thread};
use tracing::*;
use tracing_core::Dispatch;
use tracing_subscriber::{layer::SubscriberExt, reload, EnvFilter};

pub fn create_subscriber<S: AsRef<str>>(
    endpoint: S,
    filter: S,
) -> Result<
    (
        Dispatch,
        Box<dyn Fn(EnvFilter) -> Result<(), tracing_subscriber::reload::Error> + Send + Sync>,
    ),
    (),
> {
    let exporter = opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint(endpoint.as_ref().parse().unwrap())
        .with_process(opentelemetry_jaeger::Process {
            service_name: "min-repr".to_string(),
            tags: Vec::new(),
        })
        .init()
        .unwrap();
    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::Always),
            ..Default::default()
        })
        .build();
    let tracer = provider.get_tracer("tracing");

    let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let (filter, handle) = reload::Layer::new(EnvFilter::new(filter));

    let subscriber = tracing_subscriber::registry()
        .with(opentelemetry)
        .with(filter);

    let dispatch = Dispatch::new(subscriber);

    let change_level = Box::new(move |new_filter| handle.clone().reload(new_filter));

    Ok((dispatch, change_level))
}

fn main() {
    let filter = "warn";
    let (dispatch, reload) =
        create_subscriber("127.0.0.1:6831", filter).expect("create subscriber");
    let d2 = dispatch.clone();
    tracing::dispatcher::set_global_default(d2).expect("set default dispatch");

    // Nothing here is printed (filter is at warn)
    {
        let s = info_span!("0-doesn't print-thread1");
        let _g = s.enter();
        trace!("trace");
        debug!("debug");
        info!("info");
    }
    let reload = Arc::new(reload);

    let reload2 = reload.clone();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // Reload to `trace` level filter
        (reload2)(EnvFilter::new("trace")).expect("reload filter");
        tx.send(()).expect("send complete signal");

        // All of these should emit. They do.
        {
            let s = info_span!("1-always-thread2");
            let _g = s.enter();
            trace!("trace");
            debug!("debug");
            info!("info");
            warn!("warn");
            error!("error");
        }
    });
    rx.recv().expect("receive complete signal");

    // All of these should emit. They sometimes do.
    {
        let s = info_span!("2-sometimes-thread1");
        let _g = s.enter();
        trace!("trace");
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");
    }

    // Reload to `trace` level filter
    (reload.clone())(EnvFilter::new("trace")).expect("reload filter");

    let reload2 = reload.clone();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // Comment this block out to get racy behavior for step 3.
        // {
        //     let s = info_span!("2.5-sometimes-thread3");
        //     let _g = s.enter();
        //     trace!("trace");
        //     debug!("debug");
        //     info!("info");
        //     warn!("warn");
        //     error!("error");
        // }

        // Reload to `trace` level filter
        (reload2)(EnvFilter::new("trace")).expect("reload filter");
        tx.send(()).expect("send complete signal");

        // All of these should emit. They usually don't if the block above is commented out ("2.5"). They sometimes
        // don't even when that block is uncommented.
        {
            let s = info_span!("3-sometimes-thread3");
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
        let s = info_span!("4-always-thread1");
        let _g = s.enter();
        trace!("trace");
        debug!("debug");
        info!("info");
        warn!("warn");
        error!("error");
    }
}
