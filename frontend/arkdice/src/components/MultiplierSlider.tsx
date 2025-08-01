import { useState, useEffect } from "react";
import { Slider } from "@/components/ui/slider";

interface MultiplierSliderProps {
  onChange?: (multiplier: number) => void;
  value?: number;
}

const MultiplierSlider = ({
  onChange = () => {},
  value = 2,
}: MultiplierSliderProps) => {
  // Available multiplier options from the game
  const multiplierOptions = [1.05, 1.1, 1.33, 1.5, 2, 3, 10, 25, 50, 100, 1000];
  
  // Convert multiplier to slider value (0-10 for 11 options)
  const multiplierToSliderValue = (multiplier: number): number => {
    const index = multiplierOptions.findIndex(m => Math.abs(m - multiplier) < 0.01);
    return index >= 0 ? index : 4; // Default to 2x if not found
  };

  // Convert slider value to multiplier
  const sliderValueToMultiplier = (value: number): number => {
    const index = Math.round(value);
    return multiplierOptions[index] || 2;
  };

  const [sliderValue, setSliderValue] = useState<number>(
    multiplierToSliderValue(value),
  );

  // @ts-ignore
  const [displayMultiplier, setDisplayMultiplier] =
    useState<number>(value);

  // Update slider when value prop changes
  useEffect(() => {
    setSliderValue(multiplierToSliderValue(value));
    setDisplayMultiplier(value);
  }, [value]);

  // Update multiplier when slider changes
  useEffect(() => {
    const currentMultiplier = sliderValueToMultiplier(sliderValue);
    setDisplayMultiplier(currentMultiplier);
    onChange(currentMultiplier);
  }, [sliderValue, onChange]);

  // Get color based on slider position
  const getGradientColor = () => {
    const percentage = sliderValue / (multiplierOptions.length - 1);
    if (percentage < 0.33) return "from-green-500 to-yellow-500";
    if (percentage < 0.66) return "from-yellow-500 to-orange-500";
    return "from-orange-500 to-red-500";
  };

  const handleSliderChange = (value: number[]) => {
    setSliderValue(value[0]);
  };

  // Calculate position percentage for visual elements
  const getPositionPercentage = (index: number) => {
    return (index / (multiplierOptions.length - 1)) * 100;
  };

  return (
    <div className="w-full max-w-4xl mx-auto bg-gray-800 p-8 rounded-xl border border-gray-700">
      <div className="relative mb-4">
        {/* Multiplier display above thumb */}
        {/*<motion.div*/}
        {/*  className="absolute -top-14 left-0 bg-orange-600 px-4 py-2 rounded-lg text-white font-bold shadow-xl"*/}
        {/*  style={{*/}
        {/*    left: `calc(${getPositionPercentage(sliderValue)}% - 2.5rem)`,*/}
        {/*  }}*/}
        {/*  animate={{*/}
        {/*    x: 0,*/}
        {/*    opacity: 1,*/}
        {/*  }}*/}
        {/*  initial={{ opacity: 0.8 }}*/}
        {/*  transition={{ type: "spring", stiffness: 300, damping: 20 }}*/}
        {/*>*/}
        {/*  <div className="text-lg">{displayMultiplier < 10 ? displayMultiplier.toFixed(2) : displayMultiplier}x</div>*/}
        {/*  <div className="text-xs opacity-90">Win: {((1 / displayMultiplier) * 98.1).toFixed(1)}%</div>*/}
        {/*</motion.div>*/}

        {/* Slider track with gradient */}
        <div className="h-2 w-full bg-gray-800 rounded-full overflow-hidden mb-1">
          <div
            className={`h-full rounded-full bg-gradient-to-r ${getGradientColor()}`}
            style={{ width: `${getPositionPercentage(sliderValue)}%` }}
          />
        </div>

        {/* Slider component */}
        <Slider
          value={[sliderValue]}
          max={multiplierOptions.length - 1}
          step={1}
          onValueChange={handleSliderChange}
          className="mt-2"
        />

        {/* Tick marks with labels */}
        <div className="relative h-16 mt-4">
          {multiplierOptions.map((multiplier, index) => (
            <div
              key={index}
              className="absolute flex flex-col items-center"
              style={{ left: `${getPositionPercentage(index)}%`, transform: 'translateX(-50%)' }}
            >
              <div className="w-1 h-3 bg-gray-600 rounded-full" />
              <span className="text-xs text-gray-400 mt-2 whitespace-nowrap font-medium">
                {multiplier < 10 ? multiplier.toFixed(2) : multiplier}x
              </span>
              {/* Win probability */}
              <span className="text-[10px] text-gray-500 mt-0.5">
                {((1 / multiplier) * 98.1).toFixed(1)}%
              </span>
            </div>
          ))}
        </div>
      </div>

    </div>
  );
};

export default MultiplierSlider;
