<?php
//判断Minecraft皮肤模型类型
function skinapi_get_model_type($skinpath){
    $img = imagecreatefrompng($skinpath);
    $width = imagesx($img);
    $height = imagesy($img);
    if (($width == 64 && $height == 32) || ($width == 64 && $height == 64)){
        $armwidth = imagecolorat($img, 54, 20) ==  imagecolorat($img, 54, 36) ? 3 : 4;
        if ($armwidth == 3){
            return 'slim';
        }else{
            return 'default';
        }
    }else{
        return 'unknown';
    }
}
