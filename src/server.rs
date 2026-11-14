//! HTTP/1 transport with the existing Go server's per-request I/O deadlines.
use std::{
    convert::Infallible,
    future::Future,
    io,
    net::SocketAddr,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::Request,
};
use hyper::{
    body::{Body as HttpBody, Frame, SizeHint},
    server::conn::http1,
    service::service_fn,
};
use hyper_util::rt::TokioIo;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::watch,
    task::JoinSet,
    time::{Instant, Sleep},
};
use tower::ServiceExt;

use crate::config::Server;

#[derive(Clone)]
pub struct ReadBudget {
    pub deadline: Instant,
    clock: Arc<Mutex<Clock>>,
}

impl ReadBudget {
    pub fn complete(&self) {
        self.clock.lock().expect("connection clock").body_done = true;
    }
}

struct Clock {
    reading_since: Option<Instant>,
    idle: bool,
    body_done: bool,
    headers_done: bool,
    write_deadline: Option<Instant>,
}

impl Clock {
    fn finish_response(&mut self) {
        self.reading_since = None;
        self.idle = true;
        self.body_done = false;
        self.headers_done = false;
    }
}

struct DeadlineStream {
    stream: TcpStream,
    clock: Arc<Mutex<Clock>>,
    read_timeout: Duration,
    header_timeout: Duration,
    read_timer: Pin<Box<Sleep>>,
    write_timer: Pin<Box<Sleep>>,
}

impl DeadlineStream {
    fn poll_deadline(
        timer: &mut Pin<Box<Sleep>>,
        deadline: Instant,
        context: &mut Context<'_>,
    ) -> io::Result<()> {
        timer.as_mut().reset(deadline);
        if timer.as_mut().poll(context).is_ready() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "HTTP I/O deadline exceeded",
            ));
        }
        Ok(())
    }

    fn poll_write_deadline(&mut self, context: &mut Context<'_>) -> io::Result<()> {
        let deadline = self.clock.lock().expect("connection clock").write_deadline;
        if let Some(deadline) = deadline {
            Self::poll_deadline(&mut self.write_timer, deadline, context)?;
        }
        Ok(())
    }
}

impl AsyncRead for DeadlineStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let deadline = {
            let mut clock = this.clock.lock().expect("connection clock");
            if clock.body_done {
                None
            } else {
                let timeout = if clock.idle || clock.headers_done {
                    this.read_timeout
                } else {
                    this.header_timeout
                };
                Some(*clock.reading_since.get_or_insert_with(Instant::now) + timeout)
            }
        };
        if let Some(deadline) = deadline
            && let Err(error) = Self::poll_deadline(&mut this.read_timer, deadline, context)
        {
            return Poll::Ready(Err(error));
        }
        let before = buffer.filled().len();
        let result = Pin::new(&mut this.stream).poll_read(context, buffer);
        if matches!(result, Poll::Ready(Ok(()))) && buffer.filled().len() > before {
            let mut clock = this.clock.lock().expect("connection clock");
            if clock.idle {
                // Go first bounds the idle wait, then grants a new request budget.
                clock.reading_since = Some(Instant::now());
                clock.idle = false;
            }
        }
        result
    }
}

impl AsyncWrite for DeadlineStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if let Err(error) = this.poll_write_deadline(context) {
            return Poll::Ready(Err(error));
        }
        Pin::new(&mut this.stream).poll_write(context, data)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        // TcpStream has no buffered output to flush. Checking a write deadline
        // here would cancel an in-flight handler before it attempts any write.
        Pin::new(&mut self.get_mut().stream).poll_flush(context)
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(context)
    }
}

struct ResponseBody {
    inner: Body,
    clock: Arc<Mutex<Clock>>,
}

impl HttpBody for ResponseBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, axum::Error>>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_frame(context);
        if this.inner.is_end_stream() {
            this.clock
                .lock()
                .expect("connection clock")
                .finish_response();
        }
        result
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }
    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

async fn connection(
    stream: TcpStream,
    peer: SocketAddr,
    router: Router,
    config: (Duration, Duration, Duration),
    mut stop: watch::Receiver<bool>,
) {
    let (read_timeout, header_timeout, write_timeout) = config;
    let clock = Arc::new(Mutex::new(Clock {
        reading_since: Some(Instant::now()),
        idle: false,
        body_done: false,
        headers_done: false,
        write_deadline: None,
    }));
    let stream = DeadlineStream {
        stream,
        clock: clock.clone(),
        read_timeout,
        header_timeout,
        read_timer: Box::pin(tokio::time::sleep(read_timeout)),
        write_timer: Box::pin(tokio::time::sleep(write_timeout)),
    };
    let service = service_fn(move |mut request: Request<hyper::body::Incoming>| {
        let router = router.clone();
        let clock = clock.clone();
        let deadline = {
            let mut state = clock.lock().expect("connection clock");
            let deadline = *state.reading_since.get_or_insert_with(Instant::now) + read_timeout;
            state.write_deadline = Some(Instant::now() + write_timeout);
            state.body_done = request.body().is_end_stream();
            state.headers_done = true;
            state.idle = false;
            deadline
        };
        request.extensions_mut().insert(ReadBudget {
            deadline,
            clock: clock.clone(),
        });
        request.extensions_mut().insert(ConnectInfo(peer));
        async move {
            let response = router
                .oneshot(request.map(Body::new))
                .await
                .expect("router is infallible");
            if response.body().is_end_stream() {
                clock.lock().expect("connection clock").finish_response();
            }
            Ok::<_, Infallible>(response.map(|inner| ResponseBody { inner, clock }))
        }
    });
    let mut builder = http1::Builder::new();
    // The transport clock distinguishes keep-alive idle wait from header reads;
    // Hyper's single header timer would close idle connections too early.
    builder.header_read_timeout(None);
    let future = builder.serve_connection(TokioIo::new(stream), service);
    tokio::pin!(future);
    let result = tokio::select! {
        result = &mut future => result,
        _ = stop.changed() => {
            future.as_mut().graceful_shutdown();
            future.await
        }
    };
    if result.is_err() {
        tracing::debug!("HTTP connection closed with a protocol or I/O error");
    }
}

pub async fn serve(
    listener: TcpListener,
    router: Router,
    config: &Server,
    stop: impl Future<Output = Result<(), String>>,
) -> Result<(), String> {
    let mut connections = JoinSet::new();
    let (sender, receiver) = watch::channel(false);
    let timeouts = (
        config.read_timeout.0,
        config.read_header_timeout.0,
        config.write_timeout.0,
    );
    tokio::pin!(stop);
    loop {
        tokio::select! {
            stopped = &mut stop => { stopped?; break; }
            accepted = listener.accept() => {
                let (stream, peer) = accepted.map_err(|_| "cannot accept HTTP connection")?;
                connections.spawn(connection(stream, peer, router.clone(), timeouts, receiver.clone()));
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if result.is_err() { tracing::error!("HTTP connection task failed"); }
            }
        }
    }
    let _ = sender.send(true);
    tokio::time::timeout(config.shutdown_timeout.0, async {
        while let Some(result) = connections.join_next().await {
            if result.is_err() {
                tracing::error!("HTTP connection task failed during shutdown");
            }
        }
    })
    .await
    .map_err(|_| "graceful shutdown timed out")?;
    Ok(())
}
