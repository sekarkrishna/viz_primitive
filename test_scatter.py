import numpy as np
from PIL import Image
from justviz import scatter

# Generate random points
n_points = 100
x = np.random.uniform(-0.8, 0.8, n_points)
y = np.random.uniform(-0.8, 0.8, n_points)

print("Generating scatter plot...")
img_array = scatter(x, y, color=[1.0, 0.4, 0.6], size=10.0, opacity=0.8, width=800, height=600)

print("Saving to scatter_output.png")
# img_array is RGBA (800, 600, 4) in shape (height=600, width=800)
image = Image.fromarray(img_array, mode="RGBA")
image.save("scatter_output.png")
print("Done!")
