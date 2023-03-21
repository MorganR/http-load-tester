use cloud_storage::client::Client;
use goose::{prelude::*, config, logger::GooseLogFormat};
use std::{result::Result, error::Error, fs, path::{Path, PathBuf}, io, time::Duration};

const REQUEST_LOG_FORMAT: GooseLogFormat = GooseLogFormat::Csv;

async fn loadtest_strings(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get("/strings/hello").await?;
    let _goose_metrics = user.get("/strings/hello?name=cool%20gal").await?;
    let _goose_metrics = user.get("/strings/async-hello").await?;
    let _goose_metrics = user.get("/strings/lines?n=10000").await?;

    Ok(())
}

async fn loadtest_static(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get("/static/basic.html").await?;
    let _goose_metrics = user.get("/static/scout.webp").await?;

    Ok(())
}

async fn loadtest_math(user: &mut GooseUser) -> TransactionResult {
    let _goose_metrics = user.get("/math/power-reciprocals-alt?n=1000").await?;
    let _goose_metrics = user.get("/math/power-reciprocals-alt?n=10000000").await?;

    Ok(())
}

fn compute_logs_path(maybe_log_dir: &Option<String>, maybe_report_name: &Option<String>) -> PathBuf {
    let mut path_buf = match maybe_log_dir {
        Some(ld) => Path::new(ld),
        _ => Path::new("/tmp")
    }.to_path_buf();

    if let Some(report_name) = maybe_report_name {
        path_buf.push(report_name);
    };
    path_buf
}

fn maybe_prep_log_dir(maybe_log_dir: &Option<String>, maybe_report_name: &Option<String>) -> io::Result<()> {
    let dir_path = compute_logs_path(maybe_log_dir, maybe_report_name);
    if !dir_path.exists() {
        return fs::create_dir_all(dir_path);
    }
    Ok(())
}

fn report_log_path(maybe_log_dir: &Option<String>, maybe_report_name: &Option<String>, iteration: usize) -> PathBuf {
    log_path(maybe_log_dir, maybe_report_name, iteration, "report.html")
}

fn request_log_path(maybe_log_dir: &Option<String>, maybe_report_name: &Option<String>, iteration: usize) -> PathBuf {
    log_path(maybe_log_dir, maybe_report_name, iteration, "requests.csv")
}

fn log_path(maybe_log_dir: &Option<String>, maybe_report_name: &Option<String>, iteration: usize, suffix: &str) -> PathBuf {
    let mut path_buf = compute_logs_path(maybe_log_dir, maybe_report_name);
    path_buf.push(format!("{}-{}", iteration, suffix));
    path_buf
}

async fn maybe_copy_to_gcs(
    maybe_bucket_name: &Option<String>,
    maybe_report_name: &Option<String>,
    maybe_log_dir: &Option<String>,
    iteration: usize) -> Result<(), Box<dyn Error>> {
    let bucket_name = match maybe_bucket_name {
        Some(b) => b,
        _ => return Ok(())
    };
    if bucket_name.is_empty() {
        return Ok(())
    }
    let client = Client::new();

    let report_path = report_log_path(maybe_log_dir, maybe_report_name, iteration);
    let f = fs::read(&report_path)?;
    client.object().create(
        bucket_name.as_str(),
        f,
        report_path.file_name().unwrap().to_str().unwrap(),
        "text/html").await?;

    let request_csv_path = request_log_path(maybe_log_dir, maybe_report_name, iteration);
    let f = fs::read(&request_csv_path)?;
    client.object().create(
        bucket_name.as_str(),
        f,
        request_csv_path.file_name().unwrap().to_str().unwrap(),
        "text/csv").await?;

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
    };

    let iterations_end = match options.iterations {
        Some(i) => i + 1,
        _ => 2
    };

    maybe_prep_log_dir(&options.log_dir, &options.report_name)?;

    for i in 1..iterations_end {
        println!("Commencing iteration {}", i);
        let mut configuration = config::GooseConfiguration::default();
        configuration.host = options.host.clone();
        configuration.report_file = report_log_path(&options.log_dir, &options.report_name, i).to_str().unwrap().to_string();
        // Max 10 second timeout per request.
        configuration.timeout = Some("10".to_string());
        configuration.users = Some(options.users);
        configuration.startup_time = options.start_time.clone();
        configuration.run_time = options.run_time.clone();
        configuration.request_log = request_log_path(&options.log_dir, &options.report_name, i).to_str().unwrap().to_string();
        configuration.request_format = Some(REQUEST_LOG_FORMAT);

        GooseAttack::initialize_with_config(configuration)?
            .register_scenario(scenario!("LoadtestTransactions")
                .register_transaction(transaction!(loadtest_strings))
                .register_transaction(transaction!(loadtest_static))
                .register_transaction(transaction!(loadtest_math))
            )
            .execute()
            .await?;

        maybe_copy_to_gcs(&options.bucket, &options.report_name, &options.log_dir, i).await?;
        println!("Completed iteration {}", i);

        if i < (iterations_end - 1) {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }

    Ok(()) 
}
