<?php
require_once $_SERVER['DOCUMENT_ROOT'] ."/includes/functions.php";
$request_uri = $_SERVER['REQUEST_URI'];
$method = $_SERVER['REQUEST_METHOD'];
$ip = $_SERVER['REMOTE_ADDR'];

switch ($request_uri) {
    case '/':
        header('Content-Type: application/json; charset=utf-8');
        $data = array(
            'message' => 'This is AI Hobbyist MC API, please use the correct endpoint to access the API.'
        );
        echo json_encode($data, JSON_UNESCAPED_UNICODE|JSON_PRETTY_PRINT);
        break;
    case '/mcauth':
        if ($method == 'GET'){
            header('Content-Type: application/json; charset=utf-8');
            $data = mcauth_init();
            echo $data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/authserver/authenticate':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $login_data = mcauth_authenticate($data['username'], $data['password'], $data['clientToken'], $data['requestUser'], $data['agent']);
            echo $login_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/authserver/refresh':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $refresh_data = refresh_token($data['accessToken'], $data['clientToken'], $data['requestUser']);
            echo $refresh_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/authserver/validate':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $validate_data = validate_token($data['accessToken'], $data['clientToken']);
            echo $validate_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/authserver/invalidate':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $invalidate_data = invalidate_token($data['accessToken'], $data['clientToken']);
            echo $invalidate_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/authserver/signout':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $signout_data = signout($data['username'], $data['password']);
            echo $signout_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case strpos($request_uri, '/mcauth/sessionserver/session/minecraft/profile') === 0:
        if ($method == 'GET'){
            header('Content-Type: application/json; charset=utf-8');
            preg_match('/profile\/([a-f0-9]{32})/', $request_uri, $matches);
            $uuid = $matches[1];

            $parsed_url = parse_url($request_uri); 
            $query_params = []; 
            parse_str($parsed_url['query'] ?? '', $query_params); 
            $unsigned = $query_params['unsigned'] ?? null;

            $profile = get_profile($uuid, $unsigned);
            echo $profile;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/api/profiles/minecraft':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $profiles = get_profiles($data);
            echo $profiles;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case '/mcauth/sessionserver/session/minecraft/join':
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $join_data = join_server($data['serverId'], $data['accessToken'], $data['selectedProfile'], $ip);
            echo $join_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case strpos($request_uri, '/mcauth/sessionserver/session/minecraft/hasJoined') === 0:
        if ($method == 'GET'){
            header('Content-Type: application/json; charset=utf-8');
            $parsed_url = parse_url($request_uri);
            $query_params = []; 
            parse_str($parsed_url['query'] ?? '', $query_params); 
            $username = $query_params['username'] ?? null;
            $username = urldecode($username);
            $serverId = $query_params['serverId'] ?? null;
            $clientIp = $query_params['ip'] ?? $ip;
            $hasJoined = validate_client($username, $serverId, $clientIp);
            echo $hasJoined;
        } else {
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case strpos($request_uri, '/helm/') === 0:
        if ($method == 'GET'){
            header('Content-Type: image/png');
            $parsed_url = parse_url($request_uri); 
            $path = $parsed_url['path'];
            $parts = explode('/', trim($path, '/'));
            $uuid = $parts[1];
            $size = $parts[2];
            $avatar = get_avatar($uuid, $size);
            echo $avatar;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case strpos($request_uri, '/mcauth/users/profiles/minecraft/') === 0 || strpos($request_uri, '/mcauth/api/users/profiles/minecraft/') === 0:
        if ($method == 'GET'){
            header('Content-Type: application/json; charset=utf-8');
            $parsed_url = parse_url($request_uri); 
            $path = $parsed_url['path'];
            $username = basename($path);
            $username = urldecode($username);
            $user = mcauth_user_api($username);
            echo $user;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    case "/system/deluser":
        if ($method == 'POST'){
            header('Content-Type: application/json; charset=utf-8');
            $data = json_decode(file_get_contents('php://input'), true);
            $deluser_data = mcauth_del_user($data['super_token'], $data['prarms']['uid']);
            echo $deluser_data;
        }else{
            echo Exceptions::doErr(405, 'HTTP/1.1 405 Method Not Allowed', '不支持的请求方法');
        }
        break;
    default:
        header('content-type:application/json;charset=utf8');
        echo mcauth_init();
        break;
}
