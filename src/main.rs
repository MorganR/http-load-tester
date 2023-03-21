use cloud_storage::client::Client;
use goose::{config, logger::GooseLogFormat, prelude::*};
use std::{
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    result::Result,
    time::Duration,
};

const REQUEST_LOG_FORMAT: GooseLogFormat = GooseLogFormat::Csv;
static APP_USER_AGENT: &str = "http-load-tester/0.0.1";

async fn configure_user_without_compression(user: &mut GooseUser) -> TransactionResult {
    let builder = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .cookie_store(true)
        .no_brotli()
        .no_gzip()
        .timeout(Duration::from_secs(10));
    user.set_client_builder(builder).await?;
    Ok(())
}

async fn configure_user_with_compression(user: &mut GooseUser) -> TransactionResult {
    let builder = reqwest::Client::builder()
        .user_agent(APP_USER_AGENT)
        .cookie_store(true)
        .brotli(true)
        .gzip(true)
        .timeout(Duration::from_secs(10));
    user.set_client_builder(builder).await?;
    Ok(())
}

async fn loadtest_strings(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get_named("/strings/hello", "hello").await?;
    let _goose_metrics = user
        .get_named("/strings/hello?name=cool%20gal", "hello-param")
        .await?;
    let _goose_metrics = user.get_named("/strings/hello?name=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "hello-compressed").await?;
    let _goose_metrics = user
        .get_named("/strings/async-hello", "async-hello")
        .await?;
    let _goose_metrics = user.get_named("/strings/lines?n=10000", "lines").await?;

    Ok(())
}

async fn loadtest_static(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get_named("/static/basic.html", "basic-html").await?;
    let _goose_metrics = user.get_named("/static/scout.webp", "scout-img").await?;

    Ok(())
}

async fn loadtest_math(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user
        .get_named("/math/power-reciprocals-alt?n=1000", "power-sum-easy")
        .await?;
    let _goose_metrics = user
        .get_named("/math/power-reciprocals-alt?n=10000000", "power-sum-hard")
        .await?;

    Ok(())
}

fn compute_logs_path(
    maybe_log_dir: &Option<String>,
    maybe_report_name: &Option<String>,
) -> PathBuf {
    let mut path_buf = match maybe_log_dir {
        Some(ld) => Path::new(ld),
        _ => Path::new("/tmp"),
    }
    .to_path_buf();

    if let Some(report_name) = maybe_report_name {
        path_buf.push(report_name);
    };
    path_buf
}

fn maybe_prep_log_dir(
    maybe_log_dir: &Option<String>,
    maybe_report_name: &Option<String>,
) -> io::Result<()> {
    let dir_path = compute_logs_path(maybe_log_dir, maybe_report_name);
    if !dir_path.exists() {
        return fs::create_dir_all(dir_path);
    }
    Ok(())
}

fn report_log_path(
    maybe_log_dir: &Option<String>,
    maybe_report_name: &Option<String>,
    iteration: usize,
    compressed: bool,
) -> PathBuf {
    let suffix = if compressed {
        "compressed-report.html"
    } else {
        "report.html"
    };
    log_path(maybe_log_dir, maybe_report_name, iteration, suffix)
}

fn request_log_path(
    maybe_log_dir: &Option<String>,
    maybe_report_name: &Option<String>,
    iteration: usize,
    compressed: bool,
) -> PathBuf {
    let suffix = if compressed {
        "compressed-requests.csv"
    } else {
        "requests.csv"
    };
    log_path(maybe_log_dir, maybe_report_name, iteration, suffix)
}

fn log_path(
    maybe_log_dir: &Option<String>,
    maybe_report_name: &Option<String>,
    iteration: usize,
    suffix: &str,
) -> PathBuf {
    let mut path_buf = compute_logs_path(maybe_log_dir, maybe_report_name);
    path_buf.push(format!("{}-{}", iteration, suffix));
    path_buf
}

async fn maybe_copy_to_gcs(
    maybe_bucket_name: &Option<String>,
    maybe_report_name: &Option<String>,
    maybe_log_dir: &Option<String>,
    iteration: usize,
    compressed: bool,
) -> Result<(), Box<dyn Error>> {
    let bucket_name = match maybe_bucket_name {
        Some(b) => b,
        _ => return Ok(()),
    };
    if bucket_name.is_empty() {
        return Ok(());
    }
    let client = Client::new();

    let report_path = report_log_path(maybe_log_dir, maybe_report_name, iteration, compressed);
    let f = fs::read(&report_path)?;
    client
        .object()
        .create(
            bucket_name.as_str(),
            f,
            report_path.file_name().unwrap().to_str().unwrap(),
            "text/html",
        )
        .await?;

    let request_csv_path =
        request_log_path(maybe_log_dir, maybe_report_name, iteration, compressed);
    let f = fs::read(&request_csv_path)?;
    client
        .object()
        .create(
            bucket_name.as_str(),
            f,
            request_csv_path.file_name().unwrap().to_str().unwrap(),
            "text/csv",
        )
        .await?;

    Ok(())
}

async fn run_attack(
    config: &config::GooseConfiguration,
    log_dir: &Option<String>,
    report_name: &Option<String>,
    bucket: &Option<String>,
    num_iterations: usize,
    compressed: bool,
) -> Result<(), Box<dyn Error>> {
    maybe_prep_log_dir(log_dir, report_name)?;

    for i in 1..=num_iterations {
        println!("Commencing iteration {}", i);
        let mut config = config.clone();
        config.report_file = report_log_path(log_dir, report_name, i, compressed)
            .to_str()
            .unwrap()
            .to_string();
        config.request_log = request_log_path(log_dir, report_name, i, compressed)
            .to_str()
            .unwrap()
            .to_string();

        let mut attack = GooseAttack::initialize_with_config(config)?;
        if compressed {
            attack = attack.register_scenario(
                scenario!("WithCompression")
                    .register_transaction(
                        transaction!(configure_user_with_compression).set_on_start(),
                    )
                    .register_transaction(transaction!(loadtest_strings).set_name("strings"))
                    .register_transaction(transaction!(loadtest_static).set_name("static"))
                    .register_transaction(transaction!(loadtest_math).set_name("math")),
            );
        } else {
            attack = attack.register_scenario(
                scenario!("NoCompression")
                    .register_transaction(
                        transaction!(configure_user_without_compression).set_on_start(),
                    )
                    .register_transaction(transaction!(loadtest_strings).set_name("strings"))
                    .register_transaction(transaction!(loadtest_static).set_name("static"))
                    .register_transaction(transaction!(loadtest_math).set_name("math")),
            );
        }
        attack.execute().await?;

        maybe_copy_to_gcs(bucket, report_name, log_dir, i, compressed).await?;
        println!("Completed iteration {}", i);

        if i < num_iterations {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Parsing load test args");
    let options = xflags::parse_or_exit! {
        /// Host to load test (e.g. http://localhost:8080).
        required --host h: String
        /// The number of users to hatch.
        required --users u: usize
        /// The start time over which users are hatched (e.g. 10s, 20m).
        required --start_time st: String
        /// The steady state run time, after all users are hatched (e.g. 10s, 20m).
        required --run_time rt: String
        /// The GCS bucket to write an HTML report file to, if any.
        optional --bucket b: String
        /// An optional subdirectory for metrics within the log_dir and the bucket, e.g. {report_name}/report.html.
        optional --report_name rn: String
        /// The local directory to write metrics to. Uses /tmp/ if unset. A subdirectory may be added via --report_name.
        optional --log_dir ld: String
        /// Number of iterations to run. Defaults to 1.
        optional --iterations i: usize
        /// Whether or not to enable compression.
        optional --compress c: bool
    };

    let num_iterations = match options.iterations {
        Some(i) => i,
        _ => 1,
    };

    let mut configuration = config::GooseConfiguration::default();
    configuration.host = options.host.clone();
    configuration.users = Some(options.users);
    configuration.startup_time = options.start_time.clone();
    configuration.run_time = options.run_time.clone();
    configuration.request_format = Some(REQUEST_LOG_FORMAT);

    run_attack(
        &configuration,
        &options.log_dir,
        &options.report_name,
        &options.bucket,
        num_iterations,
        false,
    )
    .await?;

    tokio::time::sleep(Duration::from_secs(10)).await;

    run_attack(
        &configuration,
        &options.log_dir,
        &options.report_name,
        &options.bucket,
        num_iterations,
        true,
    )
    .await?;

    Ok(())
}
