package com.example.macxtreamer

import android.content.Intent
import android.os.Bundle
import androidx.tv.material.ExperimentalTvMaterialApi
import androidx.tv.material.Card
import coil.compose.AsyncImage
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusable
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.tv.foundation.lazy.list.TvLazyColumn
import androidx.tv.foundation.lazy.list.items
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONArray

@OptIn(ExperimentalTvMaterialApi::class)
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme { App() }
        }
    }
}

@Composable
fun App() {
    var selectedTab by remember { mutableStateOf(0) }
    val tabs = listOf("Live", "VOD", "Series")
    var address by remember { mutableStateOf("") }
    var username by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    data class ItemUi(
        val id: String,
        val name: String,
        val coverUrl: String?,
        val extension: String?,
        val kind: String?,
        val subtitle: String?
    )
    var cats by remember { mutableStateOf(listOf<Pair<String,String>>()) } // (id,name)
    var items by remember { mutableStateOf(listOf<ItemUi>()) }
    var episodes by remember { mutableStateOf(listOf<Triple<String,String,String>>()) } // (episodeId, name, ext)
    var lastVodCat by remember { mutableStateOf<String?>(null) }
    var lastLiveCat by remember { mutableStateOf<String?>(null) }
    var lastSeriesCat by remember { mutableStateOf<String?>(null) }
    val context = LocalContext.current

    val snackbarHostState = remember { SnackbarHostState() }
    var loadingCats by remember { mutableStateOf(false) }
    var loadingItems by remember { mutableStateOf(false) }
    var loadingEpisodes by remember { mutableStateOf(false) }
    var lastError by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    LaunchedEffect(lastError) {
        lastError?.let { msg ->
            scope.launch { snackbarHostState.showSnackbar(message = msg) }
            // reset after showing once
            lastError = null
        }
    }

    Scaffold(snackbarHost = { SnackbarHost(snackbarHostState) }) { padding ->
        Column(Modifier.fillMaxSize().padding(16.dp).padding(padding)) {
            TabRow(selectedTabIndex = selectedTab) {
            tabs.forEachIndexed { i, title ->
                Tab(selected = selectedTab == i, onClick = { selectedTab = i }, text = { Text(title) })
            }
            }
            Spacer(Modifier.height(16.dp))
    if (selectedTab == 1) { // VOD
            OutlinedTextField(address, { address = it }, label = { Text("Address") })
            OutlinedTextField(username, { username = it }, label = { Text("Username") })
            OutlinedTextField(password, { password = it }, label = { Text("Password") })
            Row { 
                Button(onClick = { Jni.setConfig(address, username, password) }) { Text("Save config") }
                Spacer(Modifier.width(12.dp))
                Button(enabled = !loadingCats, onClick = {
                    loadingCats = true
                    try {
                        val json = Jni.fetchVodCategoriesJson()
                        val arr = JSONArray(json)
                        cats = (0 until arr.length()).map { idx ->
                            val o = arr.getJSONObject(idx)
                            (o.optString("id"), o.optString("name"))
                        }
                        // Auto-load last category if available
                        lastVodCat?.let { cid ->
                            cats.find { it.first == cid }?.let {
                                val json2 = Jni.fetchVodItemsJson(cid)
                                val arr2 = JSONArray(json2)
                                items = (0 until arr2.length()).map { i ->
                                    val o2 = arr2.getJSONObject(i)
                                    val ext = o2.optString("container_extension", null)
                                    val kind = when {
                                        o2.has("stream_url") -> "Movie"
                                        else -> null
                                    }
                                    ItemUi(
                                        id = o2.optString("id"),
                                        name = o2.optString("name"),
                                        coverUrl = o2.optString("cover_url", null),
                                        extension = if (ext.isNullOrBlank()) null else ext,
                                        kind = kind,
                                        subtitle = ext
                                    )
                                }
                            }
                        }
                    } catch (e: Exception) {
                        lastError = "VOD Kategorien laden fehlgeschlagen"
                    } finally { loadingCats = false }
                }) { Text("Fetch VOD Cats") }
            }
            if (loadingCats) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Kategorien…") } }
            Spacer(Modifier.height(12.dp))
            if (cats.isEmpty() && !loadingCats) { Text("Keine Kategorien", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(cats) { (id, name) ->
                            TvListCard(label = name, enabled = !loadingItems) {
                        loadingItems = true
                        try {
                            val json = Jni.fetchVodItemsJson(id)
                            val arr = JSONArray(json)
                            items = (0 until arr.length()).map { idx ->
                                val o = arr.getJSONObject(idx)
                                val ext = o.optString("container_extension", null)
                                val kind = when {
                                    o.has("stream_url") -> "Movie"
                                    else -> null
                                }
                                ItemUi(
                                    id = o.optString("id"),
                                    name = o.optString("name"),
                                    coverUrl = o.optString("cover_url", null),
                                    extension = if (ext.isNullOrBlank()) null else ext,
                                    kind = kind,
                                    subtitle = ext
                                )
                            }
                            lastVodCat = id
                        } catch (e: Exception) {
                            lastError = "VOD Inhalte laden fehlgeschlagen"
                        } finally { loadingItems = false }
                    }
                }
            }
            if (loadingItems) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Inhalte…") } }
            Spacer(Modifier.height(12.dp))
            Text("Items:")
            if (items.isEmpty() && !loadingItems) { Text("Keine Items", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(items) { it ->
                    TvListCard(label = it.name, imageUrl = it.coverUrl, subtitle = it.subtitle, kind = it.kind, extension = it.extension, enabled = !loadingItems) {
                        val url = Jni.buildStreamUrl("Movie", it.id)
                        val intent = Intent(context, PlayerActivity::class.java).apply { putExtra("url", url) }
                        context.startActivity(intent)
                    }
                }
            }
        } else if (selectedTab == 0) { // Live
            Row {
                Button(onClick = { Jni.setConfig(address, username, password) }) { Text("Save config") }
                Spacer(Modifier.width(12.dp))
                Button(enabled = !loadingCats, onClick = {
                    loadingCats = true
                    try {
                        val json = Jni.fetchLiveCategoriesJson()
                        val arr = JSONArray(json)
                        cats = (0 until arr.length()).map { idx ->
                            val o = arr.getJSONObject(idx)
                            (o.optString("id"), o.optString("name"))
                        }
                        lastLiveCat?.let { cid ->
                            cats.find { it.first == cid }?.let {
                                val json2 = Jni.fetchLiveItemsJson(cid)
                                val arr2 = JSONArray(json2)
                                items = (0 until arr2.length()).map { i ->
                                    val o2 = arr2.getJSONObject(i)
                                    val ext = o2.optString("container_extension", null)
                                    ItemUi(
                                        id = o2.optString("id"),
                                        name = o2.optString("name"),
                                        coverUrl = o2.optString("cover_url", null),
                                        extension = if (ext.isNullOrBlank()) null else ext,
                                        kind = "Live",
                                        subtitle = ext
                                    )
                                }
                            }
                        }
                    } catch (e: Exception) {
                        lastError = "Live Kategorien laden fehlgeschlagen"
                    } finally { loadingCats = false }
                }) { Text("Fetch Live Cats") }
            }
            if (loadingCats) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Kategorien…") } }
            Spacer(Modifier.height(12.dp))
            if (cats.isEmpty() && !loadingCats) { Text("Keine Kategorien", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(cats) { (id, name) ->
                            TvListCard(label = name, enabled = !loadingItems) {
                        loadingItems = true
                        try {
                            val json = Jni.fetchLiveItemsJson(id)
                            val arr = JSONArray(json)
                            items = (0 until arr.length()).map { idx ->
                                val o = arr.getJSONObject(idx)
                                val ext = o.optString("container_extension", null)
                                ItemUi(
                                    id = o.optString("id"),
                                    name = o.optString("name"),
                                    coverUrl = o.optString("cover_url", null),
                                    extension = if (ext.isNullOrBlank()) null else ext,
                                    kind = "Live",
                                    subtitle = ext
                                )
                            }
                            lastLiveCat = id
                        } catch (e: Exception) {
                            lastError = "Live Inhalte laden fehlgeschlagen"
                        } finally { loadingItems = false }
                    }
                }
            }
            if (loadingItems) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Inhalte…") } }
            Spacer(Modifier.height(12.dp))
            Text("Channels:")
            if (items.isEmpty() && !loadingItems) { Text("Keine Channels", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(items) { it ->
                    TvListCard(label = it.name, imageUrl = it.coverUrl, subtitle = it.subtitle, kind = it.kind, extension = it.extension, enabled = !loadingItems) {
                        val url = Jni.buildStreamUrl("Live", it.id)
                        val intent = Intent(context, PlayerActivity::class.java).apply { putExtra("url", url) }
                        context.startActivity(intent)
                    }
                }
            }
        } else { // Series
            Row {
                Button(onClick = { Jni.setConfig(address, username, password) }) { Text("Save config") }
                Spacer(Modifier.width(12.dp))
                Button(enabled = !loadingCats, onClick = {
                    loadingCats = true
                    try {
                        val json = Jni.fetchSeriesCategoriesJson()
                        val arr = JSONArray(json)
                        cats = (0 until arr.length()).map { idx ->
                            val o = arr.getJSONObject(idx)
                            (o.optString("id"), o.optString("name"))
                        }
                        lastSeriesCat?.let { cid ->
                            cats.find { it.first == cid }?.let {
                                val json2 = Jni.fetchSeriesItemsJson(cid)
                                val arr2 = JSONArray(json2)
                                items = (0 until arr2.length()).map { i ->
                                    val o2 = arr2.getJSONObject(i)
                                    val ext = o2.optString("container_extension", null)
                                    ItemUi(
                                        id = o2.optString("id"),
                                        name = o2.optString("name"),
                                        coverUrl = o2.optString("cover_url", null),
                                        extension = if (ext.isNullOrBlank()) null else ext,
                                        kind = "Series",
                                        subtitle = ext
                                    )
                                }
                            }
                        }
                    } catch (e: Exception) {
                        lastError = "Serien-Kategorien laden fehlgeschlagen"
                    } finally { loadingCats = false }
                }) { Text("Fetch Series Cats") }
            }
            if (loadingCats) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Kategorien…") } }
            Spacer(Modifier.height(12.dp))
            if (cats.isEmpty() && !loadingCats) { Text("Keine Kategorien", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(cats) { (id, name) ->
                            TvListCard(label = name, enabled = !loadingItems) {
                        loadingItems = true
                        try {
                            val json = Jni.fetchSeriesItemsJson(id)
                            val arr = JSONArray(json)
                            items = (0 until arr.length()).map { idx ->
                                val o = arr.getJSONObject(idx)
                                val ext = o.optString("container_extension", null)
                                ItemUi(
                                    id = o.optString("id"),
                                    name = o.optString("name"),
                                    coverUrl = o.optString("cover_url", null),
                                    extension = if (ext.isNullOrBlank()) null else ext,
                                    kind = "Series",
                                    subtitle = ext
                                )
                            }
                            episodes = emptyList()
                            lastSeriesCat = id
                        } catch (e: Exception) {
                            lastError = "Serien laden fehlgeschlagen"
                        } finally { loadingItems = false }
                    }
                }
            }
            if (loadingItems) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Serien…") } }
            Spacer(Modifier.height(12.dp))
            Text("Series:")
            if (items.isEmpty() && !loadingItems) { Text("Keine Serien", textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth()) }
            TvLazyColumn {
                items(items) { it ->
                    TvListCard(label = it.name, imageUrl = it.coverUrl, subtitle = it.subtitle, kind = it.kind, extension = it.extension, enabled = !loadingEpisodes) {
                        loadingEpisodes = true
                        try {
                            val json = Jni.fetchSeriesEpisodesJson(it.id)
                            val arr = JSONArray(json)
                            episodes = (0 until arr.length()).map { idx ->
                                val o = arr.getJSONObject(idx)
                                Triple(
                                    o.optString("episode_id"),
                                    o.optString("name"),
                                    o.optString("container_extension", "mp4")
                                )
                            }
                        } catch (e: Exception) {
                            lastError = "Episoden laden fehlgeschlagen"
                        } finally { loadingEpisodes = false }
                    }
                }
            }
            if (loadingEpisodes) { Row(verticalAlignment = Alignment.CenterVertically) { CircularProgressIndicator(); Spacer(Modifier.width(8.dp)); Text("Lade Episoden…") } }
            if (episodes.isNotEmpty()) {
                Spacer(Modifier.height(12.dp))
                Text("Episodes:")
                TvLazyColumn {
                    items(episodes) { (epId, epName, ext) ->
                        TvListCard(label = epName, subtitle = ext, kind = "Episode", extension = ext, enabled = true) {
                            val url = Jni.buildStreamUrlWithExt("SeriesEpisode", epId, ext)
                            val intent = Intent(context, PlayerActivity::class.java).apply { putExtra("url", url) }
                            context.startActivity(intent)
                        }
                    }
                }
            }
        }
        }
    }
    }

@Composable
fun FocusListItem(label: String, enabled: Boolean = true, onActivate: () -> Unit) {
    var focused by remember { mutableStateOf(false) }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp)
            .border(2.dp, if (focused) Color(0xFF00BCD4) else Color.Transparent, RoundedCornerShape(6.dp))
            .background(if (focused) Color(0x2200BCD4) else Color.Transparent, RoundedCornerShape(6.dp))
            .focusable(enabled)
            .onPreviewKeyEvent { ev ->
                if (ev.type == KeyEventType.KeyUp && (ev.key == Key.Enter || ev.key == Key.DirectionCenter)) {
                    onActivate(); true
                } else false
            }
            .clickable(enabled = enabled) { onActivate() }
            .onFocusChanged { st -> focused = st.isFocused }
            .padding(12.dp)
    ) {
        Text(label)
    }
}

@OptIn(ExperimentalTvMaterialApi::class)
@Composable
fun TvListCard(label: String, imageUrl: String? = null, subtitle: String? = null, kind: String? = null, extension: String? = null, enabled: Boolean = true, onActivate: () -> Unit) {
    Card(onClick = { if (enabled) onActivate() }, enabled = enabled) {
        Box(Modifier.fillMaxWidth().height(170.dp)) {
            if (!imageUrl.isNullOrBlank()) {
                AsyncImage(
                    model = imageUrl,
                    contentDescription = label,
                    modifier = Modifier.fillMaxSize()
                )
            }
            // Gradient overlay bottom
            Box(
                Modifier.fillMaxSize().background(
                    brush = androidx.compose.ui.graphics.Brush.verticalGradient(
                        0f to Color.Transparent,
                        0.6f to Color(0x80000000),
                        1f to Color(0xCC000000)
                    )
                )
            )
            Column(Modifier.align(Alignment.BottomStart).padding(12.dp)) {
                Text(label, color = Color.White)
                if (!subtitle.isNullOrBlank()) {
                    Text(subtitle, color = Color.LightGray, style = MaterialTheme.typography.bodySmall)
                }
                Row(Modifier.padding(top = 4.dp)) {
                    if (!kind.isNullOrBlank()) Badge(kind)
                    if (!extension.isNullOrBlank() && extension != kind) {
                        Spacer(Modifier.width(6.dp)); Badge(extension.uppercase())
                    }
                }
            }
        }
    }
}

@Composable
private fun Badge(text: String) {
    Box(
        modifier = Modifier.background(Color(0x6600BCD4), RoundedCornerShape(4.dp)).padding(horizontal = 6.dp, vertical = 2.dp)
    ) {
        Text(text, color = Color.White, style = MaterialTheme.typography.labelSmall)
    }
}
}
