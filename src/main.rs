use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Html,
    routing::get,
    Router,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use tokio::sync::broadcast;

const HTML_CONTENT: &str = r#"
<!DOCTYPE html>
<html>
<head><meta name="viewport" content="width=device-width, initial-scale=1.0"><title>电脑声音同步</title></head>
<body style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:80vh;font-family:sans-serif;">
  <h2>电脑系统音频实时串流</h2>
  <button id="btn" style="padding:15px 30px;font-size:18px;">点击连接并播放声音</button>
  <p id="status">未连接</p>
  <script>
    const btn = document.getElementById('btn');
    const status = document.getElementById('status');

    btn.onclick = async () => {
      // 采样率需与服务端一致（通常为 48000Hz）
      const audioCtx = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 48000 });
      await audioCtx.resume();

      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const ws = new WebSocket(`${protocol}//${location.host}/ws`);
      ws.binaryType = 'arraybuffer';
      status.innerText = '正在连接...';

      let nextStartTime = audioCtx.currentTime;

      ws.onopen = () => { status.innerText = '正在播放电脑声音...'; btn.disabled = true; };
      
      ws.onmessage = (event) => {
        const floatData = new Float32Array(event.data);
        // 创建双声道或单声道缓冲区
        const audioBuffer = audioCtx.createBuffer(2, floatData.length / 2, 48000);
        
        // 解构双声道交错数据 (Interleaved L/R)
        const leftChannel = audioBuffer.getChannelData(0);
        const rightChannel = audioBuffer.getChannelData(1);
        for (let i = 0, j = 0; i < floatData.length; i += 2, j++) {
          leftChannel[j] = floatData[i];
          rightChannel[j] = floatData[i + 1];
        }

        const source = audioCtx.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(audioCtx.destination);

        const currentTime = audioCtx.currentTime;
        if (nextStartTime < currentTime) {
          nextStartTime = currentTime + 0.02; // 缓冲区轻微防断音
        }
        source.start(nextStartTime);
        nextStartTime += audioBuffer.duration;
      };

      ws.onclose = () => { status.innerText = '连接已断开'; btn.disabled = false; };
    };
  </script>
</body>
</html>
"#;

#[tokio::main]
async fn main() {
    // 广播通道：将捕获的 PCM 数据发送给所有连接的客户端
    let (tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let tx = Arc::new(tx);
    let tx_clone = Arc::clone(&tx);

    // 启动系统音频 Loopback 捕获线程
    std::thread::spawn(move || {
        let host = cpal::default_host();
        
        // 获取默认的输出设备（扬声器/耳机）
        let device = host.default_output_device().expect("未找到默认音频输出设备");
        println!("正在捕获系统输出设备声音: {}", device.name().unwrap_or_default());

        let config = device.default_output_config().expect("获取设备默认配置失败");
        let sample_rate = config.sample_rate();
        let channels = config.channels();

        println!("音频配置: 采样率 {}, 声道数 {}", sample_rate.0, channels);

        // 构建输入流捕获输出混音 (WASAPI Loopback)
        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // 将 f32 PCM 转为 raw 字节流
                let bytes: Vec<u8> = data.iter().flat_map(|&sample| sample.to_le_bytes()).collect();
                let _ = tx_clone.send(bytes);
            },
            |err| eprintln!("音频捕获出错: {}", err),
            None,
        ).expect("无法建立音频 Loopback 流");

        stream.play().expect("启动捕获流失败");
        std::thread::park(); // 保持线程常驻
    });

    let app = Router::new()
        .route("/", get(|| async { Html(HTML_CONTENT) }))
        .route("/ws", get(move |ws: WebSocketUpgrade| {
            let tx = Arc::clone(&tx);
            async move { ws.on_upgrade(|socket| handle_socket(socket, tx)) }
        }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9911").await.unwrap();
    println!("服务已启动，请用手机在同一 Wi-Fi 下访问: http://<电脑局域网IP>:9911");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_socket(mut socket: WebSocket, tx: Arc<broadcast::Sender<Vec<u8>>>) {
    let mut rx = tx.subscribe();
    while let Ok(data) = rx.recv().await {
        if socket.send(Message::Binary(data)).await.is_err() {
            break;
        }
    }
}