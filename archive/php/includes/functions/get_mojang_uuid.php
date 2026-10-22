<?php
// 根据玩家名通过Mojang API获取UUID
function skinapi_get_mojang_uuid($playername){
    $url = 'https://api.mojang.com/users/profiles/minecraft/'.$playername;
    $data = POSTDATA::content($url);
    if ($data[0] == 200){
        $uuid = json_decode($data[1], true)['id'];
        return $uuid;
    } else {
        return null;
    }
}