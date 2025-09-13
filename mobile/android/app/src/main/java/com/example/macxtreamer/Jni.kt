package com.example.macxtreamer

object Jni {
    init { System.loadLibrary("macxtreamer_core") }
    external fun setConfig(address: String, username: String, password: String)
    external fun fetchVodCategories(): Array<String>
    external fun fetchVodCategoriesJson(): String
    external fun fetchVodItemsJson(categoryId: String): String
    external fun fetchSeriesCategoriesJson(): String
    external fun fetchSeriesItemsJson(categoryId: String): String
    external fun fetchSeriesEpisodesJson(seriesId: String): String
    external fun fetchLiveCategoriesJson(): String
    external fun fetchLiveItemsJson(categoryId: String): String
    external fun buildStreamUrl(info: String, id: String): String
    external fun buildStreamUrlWithExt(info: String, id: String, ext: String): String
}
