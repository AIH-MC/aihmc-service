<?php
class database {
    public $mysqli;
    function __construct() {
        global $host,$port,$user,$pass,$dbname;
        $iscon = mysqli_connect($host.":".$port, $user, $pass, $dbname);
        if (!$iscon) {
            echo "无法连接至MySQL数据库：".mysqli_connect_error();
            header(Exceptions::$codes[500]);
            die();
        }
        $this->mysqli = new mysqli($host.":".$port, $user, $pass, $dbname);
    }
    function query($sql) {
        $stmt = $this->mysqli->prepare($sql);
        if ($stmt == false) {
            echo "MySQL查询出错：".$this->mysqli->error;
            header(Exceptions::$codes[500]);
            return -1;
        }
        $stmt->execute();
        $ret = $stmt->get_result();
        $result = $ret->fetch_all();
        if (empty($result)) {
            return false;
        } else {
            return $result;
        }
    }
    function query_change($sql) {
        $stmt = $this->mysqli->prepare($sql);
        if ($stmt == false) {
            echo "MySQL查询出错：".$this->mysqli->error;
            header(Exceptions::$codes[500]);
            return -1;
        }
        $stmt->execute();
        $ret = $stmt->get_result();
        $result = $this->mysqli->affected_rows;
        if (empty($result)) {
            return false;
        } else {
            return $result;
        }
    }
    #============登录验证===========

    //根据uid获取站点用户名
    function get_username($uid) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE uid = '$uid'");
        if ($result) {
            return $result[0][2];
        } else {
            return false;
        }
    }

    //获取支持的皮肤站列表
    function mcauth_get_skin_sites() {
        $result = $this->query("SELECT * FROM mcauth_skin_urls");
        $sites = array();
        foreach ($result as $site) {
            $sites[] = $site[1];
        }
        return $sites;
    }

    //将 accessToken 和 clientToken 与站点用户 uid 绑定
    function mcauth_bind_tokens($uid, $playername, $accessToken, $clientToken) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE uid = '$uid'");
        $create_time = time();
        if (!$result) {
            $this->query_change("INSERT INTO mcauth_tokens (uid, player_name, access_token, client_token, create_time) VALUES ('$uid', '$playername', '$accessToken', '$clientToken', '$create_time')");
        }else{
            $this->query_change("UPDATE mcauth_tokens SET client_token = '$clientToken', create_time = '$create_time' WHERE uid = '$uid'");
        }
    }

    //将 profile_id、player_uuid 、player_name 与站点用户 uid 绑定
    function mcauth_bind_profile($uid, $playername, $profile_id, $player_uuid, $texture) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE uid = '$uid'");
        if ($result) {
            $this->query_change("UPDATE mcauth_profiles SET texture_data = '$texture' WHERE uid = '$uid'");
        } else {
            $this->query_change("INSERT INTO mcauth_profiles (uid, player_name, profile_id, player_uuid, texture_data) VALUES ('$uid', '$playername', '$profile_id', '$player_uuid', '$texture')");
        }
    }

    //更新玩家材质信息
    function mcauth_update_textures($uid, $uuid, $playername, $skin_hash, $skin_type, $cape_hash) {
        $timestamps = time();
        $result = $this->query("SELECT * FROM mcauth_textures WHERE uid = '$uid'");
        if ($result) {
            $this->query_change("UPDATE mcauth_textures SET player_name = '$playername', skin_hash = '$skin_hash', skin_type = '$skin_type', cape_hash = '$cape_hash', timestamps = '$timestamps' WHERE uid = '$uid'");
        } else {
            $this->query_change("INSERT INTO mcauth_textures (uid, uuid, player_name, skin_hash, skin_type, cape_hash, timestamps) VALUES ('$uid', '$uuid', '$playername', '$skin_hash', '$skin_type', '$cape_hash', '$timestamps')");
        }
    }

    //获取玩家材质信息
    function mcauth_get_textures($uid) {
        $result = $this->query("SELECT * FROM mcauth_textures WHERE uid = '$uid'");
        if ($result) {
            return array($result[0][7], $result[0][2], $result[0][3], $result[0][4], $result[0][5], $result[0][6]);
        } else {
            return false;
        }
    }

    //获取玩家token信息
    function mcauth_get_tokens($uid) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE uid = '$uid'");
        if ($result) {
            return array($result[0][3], $result[0][4]);
        } else {
            return false;
        }
    }

    //获取玩家profile信息
    function mcauth_get_profile($uid) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE uid = '$uid'");
        if ($result) {
            return array($result[0][2], $result[0][3], $result[0][4], $result[0][5]);
        } else {
            return false;
        }
    }

    //根据uuid获取玩家资料
    function mcauth_get_profile_by_uuid($uuid) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE player_uuid = '$uuid' OR player_name = '$uuid'");
        if ($result) {
            return array($result[0][2], $result[0][4], $result[0][5]);
        } else {
            return false;
        }
    }

    //判断令牌是否有效
    function mcauth_check_token($accessToken, $clientToken) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE access_token = '$accessToken' AND client_token = '$clientToken'");
        if ($result) {
            return true;
        } else {
            return false;
        }
    }

    //根据令牌获取用户uid
    function mcauth_get_uid($accessToken, $clientToken) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE access_token = '$accessToken' AND client_token='$clientToken'");
        return $result[0][1];
    }


    //根据角色名获取玩家UUID
    function mcauth_get_uuid($playername) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE player_name = '$playername'");
        return $result[0][4];
    }

    //根据UUID获取玩家角色名
    function mcauth_get_uuid_by_name($uuid) {
        $result = $this->query("SELECT * FROM mcauth_profiles WHERE player_uuid = '$uuid'");
        if ($result) {
            return $result[0][2];
        } else {
            return false;
        }
    }

    //刷新访问令牌
    function mcauth_refresh_token($accessToken, $clientToken) {
        $this->query_change("UPDATE mcauth_tokens SET access_token = '$accessToken' WHERE client_token = '$clientToken'");
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE access_token = '$accessToken' AND client_token = '$clientToken'");
        return array($result[0][1],$result[0][3], $result[0][4]);
    }

    //吊销令牌
    function mcauth_invalidate_token($accessToken, $clientToken) {
        $this->query_change("DELETE FROM mcauth_tokens WHERE access_token = '$accessToken' AND client_token = '$clientToken'");
    }

    //登出
    function mcauth_signout($uid) {
        $this->query_change("DELETE FROM mcauth_tokens WHERE uid = '$uid'");
    }

    //获取支持的皮肤API列表
    function textureapi_get_apis() {
        $result = $this->query("SELECT * FROM texture_apis");
        $apis = array();
        foreach ($result as $api) {
            $apis[] = $api[1];
        }
        return $apis;
    }
    #============进服验证============
    //判断accessToken是否有效
    function mcauth_check_accessToken($accessToken) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE access_token = '$accessToken'");
        if ($result) {
            return true;
        } else {
            return false;
        }
    }

    //根据accessToken获取uid
    function mcauth_get_uid_by_accessToken($accessToken) {
        $result = $this->query("SELECT * FROM mcauth_tokens WHERE access_token = '$accessToken'");
        return $result[0][1];
    }

    //根据accessToken获取uuid
    function mcauth_get_uuid_by_accessToken($accessToken) {
        //根据accessToken获取uid
        $uid = $this->mcauth_get_uid_by_accessToken($accessToken);
        //根据uid获取uuid
        $uuid = $this->mcauth_get_profile($uid)[2];
        return $uuid;
    }

    //记录accessToken，serverId，ip
    function mcauth_record_login($accessToken, $serverId, $ip) {
        $uid = $this->mcauth_get_uid_by_accessToken($accessToken);
        $playername = $this->get_username($uid);
        $time = time();
        $this->query_change("INSERT INTO mcauth_logins (server_id, access_token, playername, ip, timestamp) VALUES ('$serverId', '$accessToken', '$playername', '$ip', '$time')");
    }

    //根据username、serverId、ip验证客户端
    function mcauth_check_client($username, $serverId, $ip) {
        if($ip != null){
            $result = $this->query("SELECT * FROM mcauth_logins WHERE playername = '$username' AND server_id = '$serverId' AND ip = '$ip'");
        }else{
            $result = $this->query("SELECT * FROM mcauth_logins WHERE playername = '$username' AND server_id = '$serverId'");
        }
        if ($result) {
            return true;
        } else {
            return false;
        }
    }

    //删除指定时间前的登录记录
    function mcauth_delete_login($vaildtime, $username) {
        $time = time() - $vaildtime;
        $this->query_change("DELETE FROM mcauth_logins WHERE timestamp < '$time' AND playername = '$username'");
    }
    #============皮肤加载============
    //根据玩家名获取皮肤材质，Custom Skin Loader专用
    function csl_get_skin($playername) {
        $result = $this->query("SELECT * FROM mcauth_textures WHERE player_name = '$playername'");
        if ($result) {
            return array($result[0][3], $result[0][4], $result[0][5], $result[0][6]);
        } else {
            return false;
        }
    }

    #============自定义名字============

    //根据uuid获取名字
    function get_playername($uuid) {
        $result = $this->query("SELECT * FROM mcauth_name_redirect WHERE uuid = '$uuid'");
        if ($result) {
            return $result[0][2];
        } else {
            return false;
        }
    }

    #============危险操作============
    //删除用户
    function delete_user($uid) {
        if(!$this->mcauth_get_tokens($uid)){
            return false;
        }else{
            $userinfo = $this->mcauth_get_textures($uid);
            $this->query_change("DELETE FROM mcauth_logins WHERE playername = '$userinfo[2]'");
            $this->query_change("DELETE FROM mcauth_profiles WHERE uid = '$uid'");
            $this->query_change("DELETE FROM mcauth_tokens WHERE uid = '$uid'");
            $this->query_change("DELETE FROM mcauth_textures WHERE uid = '$uid'");
            return true;
        }
    }
}