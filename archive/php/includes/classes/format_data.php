<?php
class format_data {
    static function mcauth_init($server, $imp_name, $imp_ver, $home, $register, $non_email_login, $skins, $pub_key){
        $data = array(
            "meta" => array(
                "serverName" => $server,
                "implementationName" => $imp_name,
                "implementationVersion" => $imp_ver,
                "links" => array(
                    "homepage" => $home,
                    "register" => $register,
                ),
                "feature.non_email_login" => $non_email_login,
            ),
            "skinDomains" => $skins,
            "signaturePublickey" => $pub_key
        );
        return $data;
    }

    //Minecraft 材质信息序列化
    static function mcauth_serialize_skin($timestamp, $profile_id, $profile_name, $skin_hash, $model, $cape_hash){
        $source = "https://api.aihmc.top/textures";
        $data = array(
            "timestamp" => $timestamp,
            "profileId" => $profile_id,
            "profileName" => $profile_name
        );
        if ($skin_hash != null){
            $data["textures"] = array(
                "SKIN" => array(
                    "url" => "$source/$skin_hash",
                    "metadata" => array(
                        "model" => $model,
                    ),
                ),
            );
        }
        if ($cape_hash != null){
            $data["textures"]["CAPE"] = array(
                "url" => "$source/$cape_hash",
            );
        }
        return $data;
    }

    //Minecraft 用户信息序列化
    static function mcauth_serialize($id, $properties){
        $data = array(
            "id" => $id,
            "properties" => array(),
        );
        if ($properties != null){
            $data["properties"] = $properties;
        }
        return $data;
    }

    //Minecraft 角色信息序列化
    static function mcauth_profile_properties($name, $value, $signature){
        $data = array(
            "name" => $name,
            "value" => $value,
        );
        if ($signature != null){
            $data["signature"] = $signature;
        }
        return $data;
    }

    static function mcauth_serialize_profile($id, $name, $properties){
        $data = array(
            "id" => $id,
            "name" => $name,
        );
        if ($properties != null){
            $data["properties"] = array($properties);
        }
        return $data;
    }

    //Minecraft 登录信息序列化
    static function mcauth_serialize_login($acesstoken, $clientToken, $available, $selected, $user, $requestUser){
        $data = array(
            "accessToken" => $acesstoken,
            "clientToken" => $clientToken,
        );
        if ($available){
            $data["availableProfiles"] = array($available);
        }
        if ($selected){
            $data["selectedProfile"] = $selected;
        }
        if ($requestUser){
            $data["user"] = $user;
        }
        return $data;
    }

    //构建Custom_Skin_Loader数据
    static function csl_build($name, $skin_hash, $skin_type, $cape_hash){
        $data = array(
            "username" => $name,
        );
        if ($skin_hash != null){
            $data["skins"] = array(
                $skin_type => $skin_hash,
            );
        }
        if ($cape_hash != null){
            $data["capes"] = $cape_hash;
        }
        return $data;
    }

    //构建用户API数据
    static function mcauth_user_api($uuid, $name){
        $data = array(
            "id" => $uuid,
            "name" => $name,
        );
        return $data;
    }
}