<?php
include($_SERVER['DOCUMENT_ROOT'].'/mc_auth/includes/functions.php');
include($_SERVER['DOCUMENT_ROOT'].'/includes/classes.php');

//刷新令牌
function refresh_token($accessToken, $clientToken, $requestUser = false) {
    global $db;
    if ($db->mcauth_check_token("$accessToken", $clientToken)) {
        $newAccessToken = generate_new_accessToken($clientToken, $accessToken);

        $tokens = $db->mcauth_refresh_token($newAccessToken, $clientToken);
        $profiles = $db->mcauth_get_profile($tokens[0]);

        $user_info = format_data::mcauth_serialize($profiles[1], null);
        $profile_prop = format_data::mcauth_profile_properties('textures', $profiles[3], null);
        $profile_info = format_data::mcauth_serialize_profile($profiles[2],$profiles[0], $profile_prop);
        $profile_data = format_data::mcauth_serialize_login($tokens[1], $tokens[2], null, $profile_info, $user_info, $requestUser);
        $json_data = json_encode($profile_data, JSON_UNESCAPED_UNICODE | JSON_PRETTY_PRINT);

        return $json_data;
    } else {
        return Exceptions::doErr(403, 'ForbiddenOperationException', 'Invalid token.');
    }
}
    