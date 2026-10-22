<?php
function cropMinecraftAvatar($skinPath, $size) {
    $skin = imagecreatefrompng($skinPath);

    $avatar = imagecreatetruecolor($size, $size);

    $baseX = 8;
    $baseY = 8;
    $baseWidth = 8;
    $baseHeight = 8;

    $overlayX = 40;
    $overlayY = 8;
    $overlayWidth = 8;
    $overlayHeight = 8;

    imagecopyresampled($avatar, $skin, 0, 0, $baseX, $baseY, $size, $size, $baseWidth, $baseHeight);

    imagecopyresampled($avatar, $skin, 0, 0, $overlayX, $overlayY, $size, $size, $overlayWidth, $overlayHeight);

    ob_start();
    imagepng($avatar);
    $imageData = ob_get_contents();
    ob_end_clean();

    imagedestroy($skin);
    imagedestroy($avatar);

    return $imageData;
}
?>
