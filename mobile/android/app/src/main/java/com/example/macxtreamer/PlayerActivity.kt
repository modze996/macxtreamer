package com.example.macxtreamer

import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.Slider
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.media3.common.MediaItem
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.PlayerView

class PlayerActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val url = intent.getStringExtra("url") ?: ""
        setContent {
            MaterialTheme { PlayerScreen(url) }
        }
    }
}

@Composable
fun PlayerScreen(url: String) {
    Surface(Modifier.fillMaxSize()) {
        val context = androidx.compose.ui.platform.LocalContext.current
        val player = ExoPlayer.Builder(context).build()
        var isPlaying by remember { mutableStateOf(true) }
        var position by remember { mutableStateOf(0L) }
        var duration by remember { mutableStateOf(0L) }
        val updateTicker = remember { androidx.compose.runtime.snapshots.SnapshotStateObserver { } }

        DisposableEffect(Unit) {
            val mediaItem = MediaItem.fromUri(Uri.parse(url))
            player.setMediaItem(mediaItem)
            player.prepare()
            player.playWhenReady = true
            val listener = object : Player.Listener {
                override fun onIsPlayingChanged(playing: Boolean) {
                    isPlaying = playing
                }
            }
            player.addListener(listener)
            val ticker = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.Dispatchers.Main).launch {
                while (true) {
                    position = player.currentPosition
                    duration = player.duration.takeIf { it > 0 } ?: 0L
                    kotlinx.coroutines.delay(500)
                }
            }
            onDispose { player.release() }
        }
        Box(Modifier.fillMaxSize()) {
            AndroidPlayerView(player)
            // Simple overlay controls
            Column(
                modifier = Modifier.fillMaxWidth().align(Alignment.BottomCenter).background(Color(0x66000000)).padding(12.dp)
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(text = if (isPlaying) "Playing" else "Paused", color = Color.White)
                    Spacer(Modifier.width(12.dp))
                    Button(onClick = { if (isPlaying) player.pause() else player.play() }) {
                        Text(if (isPlaying) "Pause" else "Play")
                    }
                    Spacer(Modifier.width(12.dp))
                    Button(onClick = { player.seekBack() }) { Text("-10s") }
                    Spacer(Modifier.width(8.dp))
                    Button(onClick = { player.seekForward() }) { Text("+10s") }
                }
                Spacer(Modifier.height(8.dp))
                Slider(
                    value = if (duration > 0) position.toFloat() / duration.toFloat() else 0f,
                    onValueChange = { frac ->
                        if (duration > 0) {
                            val newPos = (frac * duration).toLong()
                            player.seekTo(newPos)
                            position = newPos
                        }
                    }
                )
            }
        }
    }
}

@Composable
fun AndroidPlayerView(player: ExoPlayer) {
    androidx.compose.ui.viewinterop.AndroidView(factory = { ctx ->
        PlayerView(ctx).apply { this.player = player }
    })
}
