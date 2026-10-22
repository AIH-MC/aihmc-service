<?php
// 获取皮肤sha256哈希值
function skinapi_get_hash($skin_url){
    $skin_data = file_get_contents($skin_url);
    $skin_hash = hash('sha256', $skin_data);
    return $skin_hash;
}