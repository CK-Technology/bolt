use bolt::network::quic::*;
use bytes::Bytes;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;
use tokio::runtime::Runtime;

fn quic_connection_establishment(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_connection");
    group.measurement_time(Duration::from_secs(10));

    for clients in [1, 10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*clients as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(clients),
            clients,
            |b, &clients| {
                b.to_async(&rt).iter(|| async move {
                    let server = QuicServer::new("127.0.0.1:0").await.unwrap();
                    let addr = server.local_addr().unwrap();

                    let mut handles = vec![];
                    for _ in 0..clients {
                        let addr = addr.clone();
                        handles.push(tokio::spawn(async move {
                            let _client = QuicClient::connect(addr).await.unwrap();
                        }));
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

fn quic_throughput_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_throughput");
    group.measurement_time(Duration::from_secs(20));

    for size in [1024, 4096, 16384, 65536, 1048576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("message_size", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async move {
                let server = QuicServer::new("127.0.0.1:0").await.unwrap();
                let addr = server.local_addr().unwrap();

                // Server task
                let server_handle = tokio::spawn(async move {
                    let mut stream = server.accept().await.unwrap();
                    while let Some(data) = stream.recv().await {
                        black_box(data);
                    }
                });

                // Client task
                let client = QuicClient::connect(addr).await.unwrap();
                let data = Bytes::from(vec![0u8; size]);

                for _ in 0..100 {
                    client.send(data.clone()).await.unwrap();
                }

                client.close().await.unwrap();
                server_handle.await.unwrap();
            });
        });
    }
    group.finish();
}

fn quic_latency_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_latency");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("ping_pong", |b| {
        b.to_async(&rt).iter(|| async {
            let server = QuicServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();

            // Server echo task
            let server_handle = tokio::spawn(async move {
                let mut stream = server.accept().await.unwrap();
                while let Some(data) = stream.recv().await {
                    stream.send(data).await.unwrap();
                }
            });

            // Client ping-pong
            let client = QuicClient::connect(addr).await.unwrap();
            let ping = Bytes::from("ping");

            for _ in 0..1000 {
                client.send(ping.clone()).await.unwrap();
                let pong = client.recv().await.unwrap();
                black_box(pong);
            }

            client.close().await.unwrap();
            server_handle.await.unwrap();
        });
    });

    group.finish();
}

fn quic_vs_tcp_comparison(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_vs_tcp");
    group.measurement_time(Duration::from_secs(15));

    let data_size = 1048576; // 1MB
    let data = Bytes::from(vec![0u8; data_size]);

    group.bench_function("quic_transfer", |b| {
        b.to_async(&rt).iter(|| async {
            let server = QuicServer::new("127.0.0.1:0").await.unwrap();
            let addr = server.local_addr().unwrap();

            let data_clone = data.clone();
            let server_handle = tokio::spawn(async move {
                let mut stream = server.accept().await.unwrap();
                stream.send(data_clone).await.unwrap();
            });

            let client = QuicClient::connect(addr).await.unwrap();
            let received = client.recv().await.unwrap();
            black_box(received);

            client.close().await.unwrap();
            server_handle.await.unwrap();
        });
    });

    group.bench_function("tcp_transfer", |b| {
        b.to_async(&rt).iter(|| async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            use tokio::net::{TcpListener, TcpStream};

            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let data_clone = data.clone();
            let server_handle = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                socket.write_all(&data_clone).await.unwrap();
            });

            let mut client = TcpStream::connect(addr).await.unwrap();
            let mut buffer = vec![0u8; data_size];
            client.read_exact(&mut buffer).await.unwrap();
            black_box(buffer);

            server_handle.await.unwrap();
        });
    });

    group.finish();
}

fn quic_multiplexing_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_multiplexing");
    group.measurement_time(Duration::from_secs(15));

    for streams in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_streams", streams),
            streams,
            |b, &streams| {
                b.to_async(&rt).iter(|| async move {
                    let server = QuicServer::new("127.0.0.1:0").await.unwrap();
                    let addr = server.local_addr().unwrap();

                    // Server handling multiple streams
                    let server_handle = tokio::spawn(async move {
                        for _ in 0..streams {
                            let stream = server.accept_stream().await.unwrap();
                            tokio::spawn(async move {
                                while let Some(data) = stream.recv().await {
                                    black_box(data);
                                }
                            });
                        }
                    });

                    // Client creating multiple streams
                    let client = QuicClient::connect(addr).await.unwrap();
                    let mut handles = vec![];

                    for _ in 0..streams {
                        let stream = client.open_stream().await.unwrap();
                        handles.push(tokio::spawn(async move {
                            let data = Bytes::from(vec![0u8; 1024]);
                            for _ in 0..100 {
                                stream.send(data.clone()).await.unwrap();
                            }
                        }));
                    }

                    for handle in handles {
                        handle.await.unwrap();
                    }

                    client.close().await.unwrap();
                    server_handle.await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn quic_packet_loss_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_recovery");
    group.measurement_time(Duration::from_secs(20));

    for loss_rate in [0.0, 0.01, 0.05, 0.1, 0.2].iter() {
        group.bench_with_input(
            BenchmarkId::new("packet_loss", format!("{}%", loss_rate * 100.0)),
            loss_rate,
            |b, &loss_rate| {
                b.to_async(&rt).iter(|| async move {
                    // Configure QUIC with simulated packet loss
                    let mut config = QuicConfig::default();
                    config.set_packet_loss_simulation(loss_rate);

                    let server = QuicServer::with_config("127.0.0.1:0", config.clone())
                        .await
                        .unwrap();
                    let addr = server.local_addr().unwrap();

                    let server_handle = tokio::spawn(async move {
                        let mut stream = server.accept().await.unwrap();
                        let mut total = 0;
                        while let Some(data) = stream.recv().await {
                            total += data.len();
                            if total >= 10_000_000 {
                                break;
                            }
                        }
                    });

                    let client = QuicClient::connect_with_config(addr, config).await.unwrap();
                    let data = Bytes::from(vec![0u8; 10000]);

                    for _ in 0..1000 {
                        client.send(data.clone()).await.unwrap();
                    }

                    client.close().await.unwrap();
                    server_handle.await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn quic_congestion_control(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_congestion");
    group.measurement_time(Duration::from_secs(20));

    for algorithm in ["cubic", "bbr", "new_reno"].iter() {
        group.bench_with_input(
            BenchmarkId::new("algorithm", algorithm),
            algorithm,
            |b, algorithm| {
                b.to_async(&rt).iter(|| async move {
                    let mut config = QuicConfig::default();
                    config.set_congestion_control(algorithm);

                    let server = QuicServer::with_config("127.0.0.1:0", config.clone())
                        .await
                        .unwrap();
                    let addr = server.local_addr().unwrap();

                    let server_handle = tokio::spawn(async move {
                        let mut stream = server.accept().await.unwrap();
                        let mut total = 0;
                        while let Some(data) = stream.recv().await {
                            total += data.len();
                            if total >= 100_000_000 {
                                break;
                            }
                        }
                    });

                    let client = QuicClient::connect_with_config(addr, config).await.unwrap();
                    let data = Bytes::from(vec![0u8; 65536]);

                    for _ in 0..1600 {
                        client.send(data.clone()).await.unwrap();
                    }

                    client.close().await.unwrap();
                    server_handle.await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn quic_encryption_overhead(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("quic_encryption");
    group.measurement_time(Duration::from_secs(15));

    for encryption in ["aes128", "aes256", "chacha20"].iter() {
        group.bench_with_input(
            BenchmarkId::new("cipher", encryption),
            encryption,
            |b, encryption| {
                b.to_async(&rt).iter(|| async move {
                    let mut config = QuicConfig::default();
                    config.set_encryption_cipher(encryption);

                    let server = QuicServer::with_config("127.0.0.1:0", config.clone())
                        .await
                        .unwrap();
                    let addr = server.local_addr().unwrap();

                    let server_handle = tokio::spawn(async move {
                        let mut stream = server.accept().await.unwrap();
                        while let Some(data) = stream.recv().await {
                            black_box(data);
                        }
                    });

                    let client = QuicClient::connect_with_config(addr, config).await.unwrap();
                    let data = Bytes::from(vec![0u8; 4096]);

                    for _ in 0..10000 {
                        client.send(data.clone()).await.unwrap();
                    }

                    client.close().await.unwrap();
                    server_handle.await.unwrap();
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    quic_connection_establishment,
    quic_throughput_benchmark,
    quic_latency_benchmark,
    quic_vs_tcp_comparison,
    quic_multiplexing_benchmark,
    quic_packet_loss_recovery,
    quic_congestion_control,
    quic_encryption_overhead
);

criterion_main!(benches);
