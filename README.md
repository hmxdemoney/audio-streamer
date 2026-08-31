# PC Audio Sync

English | [简体中文](README_zh.md)

This is a lightweight system audio synchronization tool that allows you to stream any sound playing on your computer to your phone or tablet in real-time over your local network. It is specifically optimized for iPhone and the Safari browser.

## ✨ Core Features

*   **Out of the Box**: No need to install complex client apps; just use your phone's browser to start playing.
*   **Custom Domain Access (mDNS)**: Apple device users (iPhone/iPad/Mac) don't need to type in cumbersome IP addresses. Simply visit `http://audio.local:8000` in your browser to connect.
*   **Active Silence Injection Technology**: Say goodbye to the "stuck tape" loop sound! Traditional web audio streams often loop the last tiny snippet of sound infinitely when the computer is muted or stops playing. This tool uses "Active Silence Fill" technology, proactively sending digital silence packets when the computer is quiet, ensuring a perfectly clean listening experience.
*   **Seamless Device Switching**: When you switch your audio output device on your computer (e.g., from speakers to headphones), the program automatically detects this and smoothly reconnects without needing a restart.
*   **Screen-Off Playback Support**: Perfectly supports continuous playback while the screen is off on iOS devices.

## 🚀 How to Use

1.  **Run the Program**: Execute the program on your computer. Upon starting, it will attempt to capture the current default audio output of your system.
2.  **Connect to Local Network**: Ensure that your phone (or tablet) is connected to the **same Wi-Fi network** as the computer running the program.
3.  **Open your Browser**:
    *   **For Apple Device Users (iPhone/iPad/Mac)**: Open the Safari browser and directly visit `http://audio.local:8000`
    *   **For Other Users (Android/Windows)**: Please check the IP address displayed in the console window where the program is running on your computer, and visit that address, for example, `http://192.168.x.x:8000`
4.  **Start Playing**: Click the "Connect and Play" button on the webpage on your phone to hear your computer's audio.

## 📱 Supported Devices

*   **Sender (the computer running this program)**: Supports Windows, macOS, and Linux.
*   **Receiver (the device playing the audio)**: Any device with a modern web browser. We highly recommend using **iPhone (iOS) + Safari** for the best experience.

## 💡 How it Works

After this program runs on your computer, it continuously monitors the output sound of the default system sound card in real-time. At the same time, it starts a miniature Web server and a WebSocket communication channel in the background.
When your phone's browser accesses the web page and you click play, a low-latency data channel is established between your phone and the computer. The computer converts the captured audio into a data stream and continuously sends it to your phone. The phone then uses the browser's Web Audio API to reconstruct the data stream back into sound and play it.
