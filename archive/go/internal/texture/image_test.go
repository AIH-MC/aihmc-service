package texture

import (
	"bytes"
	"encoding/json"
	"fmt"
	"image/color"
	"os"
	"path/filepath"
	"testing"

	"github.com/AIH-MC/aihmc-service/internal/testutil"
)

func TestAvatarMatchesLegacyGDPixels(t *testing.T) {
	root := testutil.Root(t)
	for _, height := range []int{32, 64} {
		data, err := os.ReadFile(filepath.Join(root, "tests/testdata", fmt.Sprintf("skin-%d.png", height)))
		if err != nil {
			t.Fatal(err)
		}
		skin, err := DecodePNG(data)
		if err != nil {
			t.Fatal(err)
		}
		for _, size := range []int{8, 16, 20, 64} {
			t.Run(fmt.Sprintf("%d_%d", height, size), func(t *testing.T) {
				actualData, err := Avatar(skin, size)
				if err != nil {
					t.Fatal(err)
				}
				expectedData, err := os.ReadFile(filepath.Join(root, "tests/testdata", fmt.Sprintf("avatar-%d-%d.png", height, size)))
				if err != nil {
					t.Fatal(err)
				}
				actual, err := DecodePNG(actualData)
				if err != nil {
					t.Fatal(err)
				}
				expected, err := DecodePNG(expectedData)
				if err != nil {
					t.Fatal(err)
				}
				if actual.Bounds() != expected.Bounds() {
					t.Fatal("avatar dimensions differ")
				}
				for y := 0; y < size; y++ {
					for x := 0; x < size; x++ {
						got := color.NRGBAModel.Convert(actual.At(x, y)).(color.NRGBA)
						want := color.NRGBAModel.Convert(expected.At(x, y)).(color.NRGBA)
						if got != want {
							t.Fatalf("pixel %d,%d: got %v, GD reference %v", x, y, got, want)
						}
					}
				}
			})
		}
	}
}

func TestModelMatchesLegacyAndRejectsCorruptPNG(t *testing.T) {
	root := testutil.Root(t)
	data, err := os.ReadFile(filepath.Join(root, "docs/legacy-image-models.json"))
	if err != nil {
		t.Fatal(err)
	}
	var models map[string]string
	if err := json.Unmarshal(data, &models); err != nil {
		t.Fatal(err)
	}
	for file, want := range models {
		data, err := os.ReadFile(filepath.Join(root, "tests/testdata", file))
		if err != nil {
			t.Fatal(err)
		}
		image, err := DecodePNG(data)
		if err != nil || ModelType(image) != want {
			t.Fatal("model differs from GD reference", file, err)
		}
		if _, err := Avatar(image, 0); err == nil {
			t.Fatal("zero avatar dimension was accepted")
		}
	}
	if _, err := DecodePNG(bytes.Repeat([]byte("broken"), 10)); err == nil {
		t.Fatal("corrupt PNG was accepted")
	}
}

func TestSlimAndTransparentHatMatchGD(t *testing.T) {
	root := testutil.Root(t)
	for _, name := range []string{"slim", "nohat"} {
		t.Run(name, func(t *testing.T) {
			data, err := os.ReadFile(filepath.Join(root, "tests/testdata/skin-"+name+".png"))
			if err != nil {
				t.Fatal(err)
			}
			skin, err := DecodePNG(data)
			if err != nil {
				t.Fatal(err)
			}
			actualData, err := Avatar(skin, 20)
			if err != nil {
				t.Fatal(err)
			}
			expectedData, err := os.ReadFile(filepath.Join(root, "tests/testdata/avatar-"+name+"-20.png"))
			if err != nil {
				t.Fatal(err)
			}
			actual, err := DecodePNG(actualData)
			if err != nil {
				t.Fatal(err)
			}
			expected, err := DecodePNG(expectedData)
			if err != nil {
				t.Fatal(err)
			}
			for y := 0; y < 20; y++ {
				for x := 0; x < 20; x++ {
					if color.NRGBAModel.Convert(actual.At(x, y)) != color.NRGBAModel.Convert(expected.At(x, y)) {
						t.Fatalf("%s differs from GD at %d,%d", name, x, y)
					}
				}
			}
		})
	}
}
