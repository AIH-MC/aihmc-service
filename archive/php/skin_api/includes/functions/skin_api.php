<?php
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

function skinapi_get_info($username) {
    global $db;
    $encoded_username = urlencode($username);
    $texture_save_path = $_SERVER['DOCUMENT_ROOT'].'/textures';
    $supported_api = $db->textureapi_get_apis();
    $skin_hash = $cape_hash = $skin_type = null;

    foreach ($supported_api as $api) {
        $csl_url = "$api/csl/$encoded_username.json";
        $csl_data = POSTDATA::content($csl_url);

        if ($csl_data[0] == 200) {
            $csl = json_decode($csl_data[1], true);
            $skin_type = isset($csl['skins']['slim']) ? 'slim' : 'default';
            $skin_hash = $csl['skins'][$skin_type] ?? null;
            $cape_hash = $csl['cape'] ?? null;

            if ($skin_hash) {
                $skin_url = "$api/textures/$skin_hash";
                file_put_contents("$texture_save_path/$skin_hash", POSTDATA::content($skin_url)[1]);
            }

            if ($cape_hash) {
                $cape_url = "$api/textures/$cape_hash";
                file_put_contents("$texture_save_path/$cape_hash", POSTDATA::content($cape_url)[1]);
            }

            if ($skin_hash || $cape_hash) {
                break; // 成功获取到皮肤或披风时跳出循环
            }
        }
    }

    // 如果没有找到皮肤或者披风，从Crafatar获取
    if (!$skin_hash || !$cape_hash) {
        list($skin_hash_craftar, $cape_hash_craftar, $skin_type_craftar) = skinapi_get_texture_from_crafatar($username, $texture_save_path);

        if (!$skin_hash) {
            $skin_hash = $skin_hash_craftar;
            $skin_type = $skin_type_craftar;
        }
        if (!$cape_hash) {
            $cape_hash = $cape_hash_craftar;
        }
    }

    return array($skin_hash, $skin_type, $cape_hash);
}
?>
