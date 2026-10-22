<?php
//从crafatar获取皮肤信息
function skinapi_get_texture_from_crafatar($username, $texture_save_path){
    $skin_hash = $cape_hash = $skin_type = null;
    $uuid = skinapi_get_mojang_uuid($username);
    $crafatar_url = "https://crafatar.com";
    $skin_url = "$crafatar_url/skins/$uuid";
    $cape_url = "$crafatar_url/capes/$uuid";
    $skin_data = POSTDATA::content($skin_url);
    $cape_data = POSTDATA::content($cape_url);
    if ($skin_data[0] == 200){
        $skin_hash = skinapi_get_hash($skin_url);
        file_put_contents("$texture_save_path/$skin_hash", $skin_data[1]);
        $skin_type = skinapi_get_model_type("$texture_save_path/$skin_hash");
    }
    if ($cape_data[0] == 200){
        $cape_hash = skinapi_get_hash($cape_url);
        file_put_contents("$texture_save_path/$cape_hash", $cape_data[1]);
    }
    return array($skin_hash, $cape_hash, $skin_type);
}