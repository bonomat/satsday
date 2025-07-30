import { useState, useEffect } from "react";
import { Slider } from "@/components/ui/slider";
import { motion } from "framer-motion";

interface MultiplierSliderProps {
  onChange?: (multiplier: number) => void;
  value?: number;
}

const MultiplierSlider = ({
  onChange = () => {},
  value = 2,
}: MultiplierSliderProps) => {
  // Convert multiplier to slider value (0-100)
  const multiplierToSliderValue = (multiplier: number): number => {
    // Logarithmic scale for better usability
    const min = Math.log(1.05);
    const max = Math.log(1000);
    const normalized = (Math.log(multiplier) - min) / (max - min);
    return normalized * 100;
  };

  // Convert slider value (0-100) to multiplier
  const sliderValueToMultiplier = (value: number): number => {
    const min = Math.log(1.05);
    const max = Math.log(1000);
    const scaled = min + (value / 100) * (max - min);
    return Math.exp(scaled);
  };

  // Common multiplier snap points (based on actual game options)
  const snapPoints = [1.05, 1.1, 1.33, 1.5, 2, 3, 10, 25, 50, 100, 1000];
  const snapPointValues = snapPoints.map(multiplierToSliderValue);

  const [sliderValue, setSliderValue] = useState<number>(
    multiplierToSliderValue(value),
  );
  const [displayMultiplier, setDisplayMultiplier] =
    useState<number>(value);
  const [isSnapping, setIsSnapping] = useState<boolean>(false);

  // Update slider when value prop changes
  useEffect(() => {
    setSliderValue(multiplierToSliderValue(value));
    setDisplayMultiplier(value);
  }, [value]);

  // Check if we should snap to a common multiplier
  useEffect(() => {
    if (isSnapping) return;

    const currentMultiplier = sliderValueToMultiplier(sliderValue);

    // Find closest snap point if we're close enough
    const snapThreshold = 3; // Adjust sensitivity as needed
    let closestSnapPoint = null;
    let minDistance = Infinity;

    for (const point of snapPointValues) {
      const distance = Math.abs(sliderValue - point);
      if (distance < minDistance && distance < snapThreshold) {
        minDistance = distance;
        closestSnapPoint = point;
      }
    }

    if (closestSnapPoint !== null) {
      setIsSnapping(true);
      setSliderValue(closestSnapPoint);
      const snappedMultiplier = sliderValueToMultiplier(closestSnapPoint);
      setDisplayMultiplier(snappedMultiplier);
      onChange(snappedMultiplier);
      setTimeout(() => setIsSnapping(false), 300);
    } else {
      setDisplayMultiplier(currentMultiplier);
      onChange(currentMultiplier);
    }
  }, [sliderValue]);

  // Get color based on multiplier value
  const getGradientColor = () => {
    const percentage = sliderValue / 100;
    if (percentage < 0.33) return "from-green-500 to-yellow-500";
    if (percentage < 0.66) return "from-yellow-500 to-orange-500";
    return "from-orange-500 to-red-500";
  };

  const handleSliderChange = (value: number[]) => {
    if (!isSnapping) {
      setSliderValue(value[0]);
    }
  };

  return (
    <div className="w-full max-w-4xl mx-auto bg-gray-900 p-6 rounded-xl">
      <div className="relative mb-10">
        {/* Multiplier display above thumb */}
        <motion.div
          className="absolute -top-12 left-0 bg-gray-800 px-3 py-1 rounded-md text-white font-bold border border-gray-700 shadow-lg"
          style={{
            left: `calc(${sliderValue}% - 2rem)`,
          }}
          animate={{
            x: 0,
            opacity: 1,
          }}
          initial={{ opacity: 0.8 }}
          transition={{ type: "spring", stiffness: 300, damping: 20 }}
        >
          {displayMultiplier.toFixed(displayMultiplier < 10 ? 2 : 1)}x
        </motion.div>

        {/* Slider track with gradient */}
        <div className="h-2 w-full bg-gray-800 rounded-full overflow-hidden mb-1">
          <div
            className={`h-full rounded-full bg-gradient-to-r ${getGradientColor()}`}
            style={{ width: `${sliderValue}%` }}
          />
        </div>

        {/* Slider component */}
        <Slider
          defaultValue={[sliderValue]}
          value={[sliderValue]}
          max={100}
          step={0.1}
          onValueChange={handleSliderChange}
          className="mt-2"
        />

        {/* Snap points indicators */}
        <div className="relative h-6 mt-1">
          {snapPoints.map((point, index) => {
            const position = multiplierToSliderValue(point);
            return (
              <div
                key={index}
                className="absolute w-1 h-3 bg-gray-600 rounded-full"
                style={{ left: `calc(${position}% - 1px)` }}
                title={`${point}x`}
              />
            );
          })}
        </div>
      </div>

      {/* Min/Max labels */}
      <div className="flex justify-between text-sm text-gray-400">
        <div>1.05x</div>
        <div>1000x</div>
      </div>
    </div>
  );
};

export default MultiplierSlider;
