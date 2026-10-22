<?php
class UUID {
    /**
     * 获取带短横线的 UUID (标准随机 v4)
     * 保留原名，内部逻辑改为随机
     */
    static function getProfileUuid($name) {
        // 调用下面的随机生成逻辑
        $uuid = self::getUserUuid($name);
        return self::createJavaUuid($uuid);
    }

    /**
     * 获取无符号的 UUID 字符串 (标准随机 v4)
     * 重要：由于是随机的，调用此函数后必须将结果存入数据库与 $name 关联
     */
    static function getUserUuid($name = null) {
        // 生成 16 字节加密安全随机数
        $data = random_bytes(16);

        // 设置版本号为 4 (0100)
        $data[6] = chr(ord($data[6]) & 0x0f | 0x40);
        // 设置变体位为 IETF (10xx)
        $data[8] = chr(ord($data[8]) & 0x3f | 0x80);

        return bin2hex($data);
    }
    
    /**
     * 格式化为标准 Java UUID 格式 (保留原名)
     */
    static function createJavaUuid($striped) {
        $components = array(
            substr($striped, 0, 8),
            substr($striped, 8, 4),
            substr($striped, 12, 4),
            substr($striped, 16, 4),
            substr($striped, 20),
        );
        return implode('-', $components);
    }

    /**
     * 去除短横线 (保留原名，修正了拼写错误但维持原名兼容)
     */
    static function unsingnedUuid($uuid) {
        return str_replace("-", "", $uuid);
    }
}