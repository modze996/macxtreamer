use jni::objects::{JClass, JString};
use jni::sys::{jobjectArray, jstring};
use jni::JNIEnv;

use crate::{fetch_categories, fetch_items, fetch_series_episodes, set_config, build_stream_url};
use serde_json::json;

fn to_string(env: &JNIEnv, js: JString) -> String {
    env.get_string(&js).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_setConfig(
    env: JNIEnv,
    _cls: JClass,
    jaddress: JString,
    juser: JString,
    jpass: JString,
) {
    let address = to_string(&env, jaddress);
    let user = to_string(&env, juser);
    let pass = to_string(&env, jpass);
    set_config(&address, &user, &pass);
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchVodCategories(
    env: JNIEnv,
    _cls: JClass,
) -> jobjectArray {
    let cats = fetch_categories("get_vod_categories").unwrap_or_default();
    let string_class = env.find_class("java/lang/String").unwrap();
    let arr = env.new_object_array(cats.len() as i32, string_class, env.new_string("").unwrap()).unwrap();
    for (i, c) in cats.iter().enumerate() {
        let s = env.new_string(&c.name).unwrap();
        env.set_object_array_element(&arr, i as i32, s).unwrap();
    }
    arr.into_inner()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchVodCategoriesJson(
    env: JNIEnv,
    _cls: JClass,
) -> jstring {
    let cats = fetch_categories("get_vod_categories").unwrap_or_default();
    let v = json!(cats);
    let s = v.to_string();
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchVodItemsJson(
    env: JNIEnv,
    _cls: JClass,
    jcategory_id: JString,
) -> jstring {
    let category_id = to_string(&env, jcategory_id);
    let items = fetch_items("vod", &category_id).unwrap_or_default();
    let v = json!(items);
    let s = v.to_string();
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_buildStreamUrl(
    env: JNIEnv,
    _cls: JClass,
    jinfo: JString,
    jid: JString,
) -> jstring {
    let info = to_string(&env, jinfo);
    let id = to_string(&env, jid);
    let url = build_stream_url(&info, &id, None);
    env.new_string(url).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_buildStreamUrlWithExt(
    env: JNIEnv,
    _cls: JClass,
    jinfo: JString,
    jid: JString,
    jext: JString,
 ) -> jstring {
    let info = to_string(&env, jinfo);
    let id = to_string(&env, jid);
    let ext = to_string(&env, jext);
    let url = build_stream_url(&info, &id, Some(&ext));
    env.new_string(url).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchSeriesCategoriesJson(
    env: JNIEnv,
    _cls: JClass,
) -> jstring {
    let cats = fetch_categories("get_series_categories").unwrap_or_default();
    let v = json!(cats);
    let s = v.to_string();
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchSeriesItemsJson(
    env: JNIEnv,
    _cls: JClass,
    jcategory_id: JString,
) -> jstring {
    let category_id = to_string(&env, jcategory_id);
    let items = fetch_items("series", &category_id).unwrap_or_default();
    let v = json!(items);
    let s = v.to_string();
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchSeriesEpisodesJson(
    env: JNIEnv,
    _cls: JClass,
    jseries_id: JString,
) -> jstring {
    let series_id = to_string(&env, jseries_id);
    let eps = fetch_series_episodes(&series_id).unwrap_or_default();
    let s = serde_json::to_string(&eps).unwrap_or("[]".to_string());
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchLiveCategoriesJson(
    env: JNIEnv,
    _cls: JClass,
) -> jstring {
    let cats = fetch_categories("get_live_categories").unwrap_or_default();
    let s = serde_json::to_string(&cats).unwrap_or("[]".to_string());
    env.new_string(s).unwrap().into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_example_macxtreamer_Jni_fetchLiveItemsJson(
    env: JNIEnv,
    _cls: JClass,
    jcategory_id: JString,
) -> jstring {
    let category_id = to_string(&env, jcategory_id);
    let items = fetch_items("live", &category_id).unwrap_or_default();
    let s = serde_json::to_string(&items).unwrap_or("[]".to_string());
    env.new_string(s).unwrap().into_raw()
}
