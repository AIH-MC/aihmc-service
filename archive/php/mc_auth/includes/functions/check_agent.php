<?php
//判断是否为Minecraft客户端
function check_agent($name){
    $name = strtolower($name);
    if ($name == "minecraft"){
        return true;
    }else{
        return false;
    }
}