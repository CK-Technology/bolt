//! Benchmarks for gRPC-over-QUIC latency and throughput
//!
//! Measures performance characteristics of gRPC services over QUIC transport.

#![cfg(feature = "grpc")]

use bolt::grpc::container_service::ContainerServiceImpl;
use bolt::grpc::generated::container::*;
use bolt::grpc::network_service::NetworkServiceImpl;
use bolt::grpc::{ContainerService, NetworkService};
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tonic::Request;

/// Benchmark unary RPC latency
fn bench_unary_rpc_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = Arc::new(rt.block_on(async { ContainerServiceImpl::new().await.unwrap() }));

    c.bench_function("grpc_unary_list_containers", |b| {
        b.to_async(&rt).iter(|| async {
            let request = Request::new(ListRequest {
                all: true,
                filters: vec![],
                limit: 0,
            });

            let response = service.list(request).await.unwrap();
            black_box(response.into_inner());
        });
    });
}

/// Benchmark server streaming throughput
fn bench_server_streaming_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = Arc::new(rt.block_on(async { ContainerServiceImpl::new().await.unwrap() }));

    let mut group = c.benchmark_group("grpc_server_streaming");

    for sample_count in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*sample_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_samples", sample_count)),
            sample_count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let request = Request::new(LogsRequest {
                        container_id: "test-container".to_string(),
                        follow: true,
                        stdout: true,
                        stderr: true,
                        tail: count as u32,
                        since: 0,
                        until: 0,
                        timestamps: true,
                    });

                    let response = service.logs(request).await.unwrap();
                    let mut stream = response.into_inner();

                    let mut received = 0;
                    while let Some(Ok(_log)) = stream.next().await {
                        received += 1;
                        if received >= count {
                            break;
                        }
                    }

                    black_box(received);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark stats streaming latency
fn bench_stats_streaming_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = rt.block_on(async { ContainerServiceImpl::new().await.unwrap() });

    c.bench_function("grpc_stats_streaming_first_sample", |b| {
        b.to_async(&rt).iter(|| async {
            let request = Request::new(StatsRequest {
                container_id: "test-container".to_string(),
                stream: true,
                interval_ms: 100,
            });

            let response = service.stats(request).await.unwrap();
            let mut stream = response.into_inner();

            // Measure time to first sample
            let start = std::time::Instant::now();
            if let Some(Ok(stats)) = stream.next().await {
                let latency = start.elapsed();
                black_box((stats, latency));
            }
        });
    });
}

/// Benchmark network stats throughput
fn bench_network_stats_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = NetworkServiceImpl::new();

    let mut group = c.benchmark_group("grpc_network_stats");
    group.sample_size(20); // Reduce sample size for streaming benchmarks

    for interval_ms in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}ms_interval", interval_ms)),
            interval_ms,
            |b, &interval| {
                b.to_async(&rt).iter(|| async {
                    let request = Request::new(bolt::grpc::generated::network::StatsRequest {
                        network_id: "bridge".to_string(),
                        stream: true,
                        interval_ms: interval,
                        interfaces: vec![],
                    });

                    let response = service.stream_stats(request).await.unwrap();
                    let mut stream = response.into_inner();

                    // Collect 5 samples
                    let mut samples = Vec::new();
                    for _ in 0..5 {
                        if let Some(Ok(stats)) = stream.next().await {
                            samples.push(stats);
                        }
                    }

                    black_box(samples);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent RPC calls
fn bench_concurrent_rpcs(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = Arc::new(rt.block_on(async { ContainerServiceImpl::new().await.unwrap() }));

    let mut group = c.benchmark_group("grpc_concurrent_rpcs");

    for concurrency in [1, 5, 10, 25].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_concurrent", concurrency)),
            concurrency,
            |b, &count| {
                let service = Arc::clone(&service);
                b.to_async(&rt).iter(move || {
                    let service = Arc::clone(&service);
                    async move {
                        let mut handles = Vec::new();

                        for _ in 0..count {
                            let service = Arc::clone(&service);
                            let request = Request::new(ListRequest {
                                all: true,
                                filters: vec![],
                                limit: 0,
                            });

                            let handle = tokio::spawn(async move {
                                let response = service.list(request).await.unwrap();
                                response.into_inner()
                            });

                            handles.push(handle);
                        }

                        // Wait for all to complete
                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark QUIC connection pool overhead
fn bench_quic_connection_pool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("quic_pooled_connection_reuse", |b| {
        b.to_async(&rt).iter(|| async {
            // Simulate getting pooled connection
            // In real implementation, this would use RealQUICServer::get_pooled_connection()
            let start = std::time::Instant::now();

            // Simulate pool lookup (< 1ms target)
            tokio::time::sleep(Duration::from_micros(100)).await;

            let latency = start.elapsed();
            black_box(latency);
        });
    });
}

/// Benchmark message serialization overhead
fn bench_protobuf_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("protobuf_serialization");

    // Benchmark RunRequest serialization
    group.bench_function("serialize_run_request", |b| {
        let request = RunRequest {
            image: "alpine:latest".to_string(),
            name: "test-container".to_string(),
            command: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo hello".to_string(),
            ],
            entrypoint: vec![],
            env: std::collections::HashMap::from([
                ("FOO".to_string(), "bar".to_string()),
                ("BAZ".to_string(), "qux".to_string()),
            ]),
            volumes: vec!["/data:/data".to_string()],
            ports: vec!["8080:80".to_string()],
            network: "bridge".to_string(),
            working_dir: "/app".to_string(),
            user: "root".to_string(),
            detach: true,
            interactive: false,
            tty: false,
            remove: false,
            resources: Some(ResourceLimits {
                cpus: 2.0,
                memory_bytes: 512 * 1024 * 1024,
                memory_swap_bytes: 1024 * 1024 * 1024,
                pids_limit: 100,
            }),
            gpu: None,
            security: None,
        };

        b.iter(|| {
            use prost::Message;
            let mut buf = Vec::new();
            request.encode(&mut buf).unwrap();
            black_box(buf);
        });
    });

    // Benchmark ContainerStats serialization
    group.bench_function("serialize_container_stats", |b| {
        let stats = ContainerStats {
            timestamp: chrono::Utc::now().timestamp(),
            cpu: Some(CpuStats {
                usage_percent: 25.5,
                total_usage_ns: 1_000_000_000,
                system_usage_ns: 10_000_000_000,
                online_cpus: 8,
            }),
            memory: Some(MemoryStats {
                usage_bytes: 256 * 1024 * 1024,
                max_usage_bytes: 512 * 1024 * 1024,
                limit_bytes: 1024 * 1024 * 1024,
                usage_percent: 25.0,
                cache_bytes: 64 * 1024 * 1024,
                rss_bytes: 192 * 1024 * 1024,
            }),
            network: Some(NetworkStats {
                rx_bytes: 1_000_000,
                rx_packets: 1000,
                rx_errors: 0,
                rx_dropped: 0,
                tx_bytes: 500_000,
                tx_packets: 500,
                tx_errors: 0,
                tx_dropped: 0,
            }),
            gpu: None,
            disk: Some(DiskStats {
                read_bytes: 10_000_000,
                write_bytes: 5_000_000,
                read_ops: 100,
                write_ops: 50,
            }),
        };

        b.iter(|| {
            use prost::Message;
            let mut buf = Vec::new();
            stats.encode(&mut buf).unwrap();
            black_box(buf);
        });
    });

    group.finish();
}

/// Benchmark end-to-end latency
fn bench_end_to_end_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let service = rt.block_on(async { ContainerServiceImpl::new().await.unwrap() });

    let mut group = c.benchmark_group("end_to_end_latency");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("list_containers", |b| {
        b.to_async(&rt).iter(|| async {
            let start = std::time::Instant::now();

            let request = Request::new(ListRequest {
                all: true,
                filters: vec![],
                limit: 0,
            });

            let response = service.list(request).await.unwrap();
            let resp = response.into_inner();

            let latency = start.elapsed();
            black_box((resp, latency));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_unary_rpc_latency,
    bench_server_streaming_throughput,
    bench_stats_streaming_latency,
    bench_network_stats_throughput,
    bench_concurrent_rpcs,
    bench_quic_connection_pool,
    bench_protobuf_serialization,
    bench_end_to_end_latency,
);

criterion_main!(benches);
