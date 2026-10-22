<?php
class chkreginfo {
    //判断用户名是否合法//
    static function chkname($name) {
        $preg = '/^[\p{Han}A-Za-z0-9_·\p{Hiragana}\p{Katakana}\p{Hangul}]{1,16}$/u';
        $res = preg_match($preg,$name);
        if (!$res) {
            return false;
        } else {
            return true;
        }
    }
    //判断密码是否合法//
    static function chkpass($pass) {
        $preg = "/^[A-Za-z0-9_~!@#$%^&*()=\[\]{}|;\',.:+-]{7,255}$/";
        $res = preg_match($preg,$pass);
        if (!$res) {
            return false;
        } else {
            return true;
        }
    }

    //判断邮箱是否合法//
    static function chkemail($email) {
        $preg = "/^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$/";
        $res = preg_match($preg,$email);
        if (!$res) {
            return false;
        } else {
            return true;
        }
    }
}