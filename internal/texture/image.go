package texture

import (
	"bytes"
	"errors"
	"image"
	"image/color"
	"image/png"
	"math"
)

func DecodePNG(data []byte) (image.Image, error) {
	return png.Decode(bytes.NewReader(data))
}

func ModelType(skin image.Image) string {
	bounds := skin.Bounds()
	if bounds.Dx() != 64 || (bounds.Dy() != 32 && bounds.Dy() != 64) {
		return "unknown"
	}
	if pixelIdentity(skin, 54, 20) == pixelIdentity(skin, 54, 36) {
		return "slim"
	}
	return "default"
}

func pixelIdentity(skin image.Image, x, y int) uint32 {
	if !image.Pt(x, y).In(skin.Bounds()) {
		// The old helper compares GD's out-of-bounds false value with integer zero.
		return 0
	}
	if palette, ok := skin.(*image.Paletted); ok {
		return uint32(palette.ColorIndexAt(x, y))
	}
	pixel := color.NRGBAModel.Convert(skin.At(x, y)).(color.NRGBA)
	alpha := 127 - uint32(pixel.A>>1)
	return alpha<<24 | uint32(pixel.R)<<16 | uint32(pixel.G)<<8 | uint32(pixel.B)
}

func Avatar(skin image.Image, size int) ([]byte, error) {
	if size <= 0 || size > math.MaxInt/size/4 {
		return nil, errors.New("invalid avatar size")
	}
	avatar := image.NewNRGBA(image.Rect(0, 0, size, size))
	for y := 0; y < size; y++ {
		for x := 0; x < size; x++ {
			pixel := color.NRGBA{A: 255}
			for _, origin := range []image.Point{{X: 8, Y: 8}, {X: 40, Y: 8}} {
				r, g, b, transparency := sample(skin, origin, x, y, size)
				opacity := 127 - transparency
				pixel.R = uint8((r*opacity + int(pixel.R)*transparency) / 127)
				pixel.G = uint8((g*opacity + int(pixel.G)*transparency) / 127)
				pixel.B = uint8((b*opacity + int(pixel.B)*transparency) / 127)
			}
			avatar.SetNRGBA(x, y, pixel)
		}
	}
	var buffer bytes.Buffer
	if err := png.Encode(&buffer, avatar); err != nil {
		return nil, err
	}
	return buffer.Bytes(), nil
}

// sample integrates the covered source area and retains GD's 7-bit alpha rounding.
// A general-purpose bilinear filter changes the legacy avatar's edge pixels.
func sample(skin image.Image, origin image.Point, x, y, size int) (int, int, int, int) {
	left, right := float64(x)*8/float64(size), float64(x+1)*8/float64(size)
	top, bottom := float64(y)*8/float64(size), float64(y+1)*8/float64(size)
	var red, green, blue, transparent, opacityWeight, area float64
	for sourceY := int(math.Floor(top)); float64(sourceY) < bottom; sourceY++ {
		height := math.Min(float64(sourceY+1), bottom) - math.Max(float64(sourceY), top)
		for sourceX := int(math.Floor(left)); float64(sourceX) < right; sourceX++ {
			width := math.Min(float64(sourceX+1), right) - math.Max(float64(sourceX), left)
			coverage := width * height
			pixel := color.NRGBA{A: 255}
			point := image.Pt(sourceX+origin.X, sourceY+origin.Y)
			if point.In(skin.Bounds()) {
				pixel = color.NRGBAModel.Convert(skin.At(point.X, point.Y)).(color.NRGBA)
			}
			alpha := float64(127 - (pixel.A >> 1))
			weight := (127 - alpha) * coverage
			red += float64(pixel.R) * weight
			green += float64(pixel.G) * weight
			blue += float64(pixel.B) * weight
			transparent += alpha * coverage
			opacityWeight += weight
			area += coverage
		}
	}
	if area != 0 {
		red, green, blue = red/area, green/area, blue/area
		transparent /= area
	}
	if opacityWeight != 0 {
		// Keep both divisions: algebraic cancellation changes rounding at half values.
		averageOpacity := opacityWeight / area
		red, green, blue = red/averageOpacity, green/averageOpacity, blue/averageOpacity
	}
	return int(math.Min(255, red+0.5)), int(math.Min(255, green+0.5)), int(math.Min(255, blue+0.5)), int(math.Min(127, transparent+0.5))
}
