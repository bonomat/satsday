import { useState, useEffect } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import MultiplierSlider from "./MultiplierSlider";
import BitcoinAddressSection from "./BitcoinAddressSection";
import ActivityFeed from "./ActivityFeed";
import Navbar from "./Navbar";
import { gameService } from "@/services/gameService";
import { type GameData, type GameAddress } from "@/types/game";
import { BETTING_CONFIG } from "@/config/betting";

// Create a local InfoDisplay component
const InfoDisplay = ({
  multiplier = 2,
}: {
  multiplier: number;
}) => {
  // Calculate potential payout based on a sample bet amount
  const sampleBetAmount = 1000; // 1,000 sats
  const potentialPayout = sampleBetAmount * multiplier;

  return (
    <div className="text-center">
      <p className="text-xl text-white">
        Send {sampleBetAmount.toLocaleString()} sats
      </p>
      <p className="text-3xl font-bold text-orange-500 mt-2">
        → Payout {Math.floor(potentialPayout).toLocaleString()} sats
      </p>
    </div>
  );
};

const Home = () => {
  const [gameData, setGameData] = useState<GameData | null>(null);
  const [selectedGame, setSelectedGame] = useState<GameAddress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const data = await gameService.fetchGameAddresses();
        setGameData(data);
        // Select the default game (2x multiplier)
        const defaultGame = data.game_addresses.find(g => g.multiplier_value === 200) || data.game_addresses[4];
        setSelectedGame(defaultGame);
        setLoading(false);
      } catch (err) {
        console.error('Failed to fetch game data:', err);
        setError('Failed to load game data');
        setLoading(false);
      }
    };
    fetchData();
  }, []);

  const handleMultiplierChange = (value: number) => {
    if (!gameData) return;
    
    // Find the closest game option based on multiplier
    const closestGame = gameData.game_addresses.reduce((prev, curr) => {
      const prevDiff = Math.abs(prev.multiplier_value / 100 - value);
      const currDiff = Math.abs(curr.multiplier_value / 100 - value);
      return currDiff < prevDiff ? curr : prev;
    });
    
    setSelectedGame(closestGame);
  };

  if (loading) {
    return (
      <div className="min-h-screen bg-gray-900 flex items-center justify-center">
        <div className="text-white text-xl">Loading game data...</div>
      </div>
    );
  }

  if (error || !gameData || !selectedGame) {
    return (
      <div className="min-h-screen bg-gray-900 flex items-center justify-center">
        <div className="text-red-500 text-xl">{error || 'Failed to load game data'}</div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-900 text-white">
      <Navbar />

      <div className="max-w-7xl mx-auto p-6">
        <header className="mb-8 text-center">
          <h1 className="text-4xl font-bold text-orange-500 mb-2">
            Select Your Odds & Win Multiplier
          </h1>
          <p className="text-gray-400">
            Send sats to the address below. If the rolled number is less than{" "}
            {selectedGame.max_roll.toLocaleString()}, you win {selectedGame.multiplier} your bet!
          </p>
        </header>

        <div className="mb-8">
          <MultiplierSlider
            value={selectedGame.multiplier_value / 100}
            onChange={handleMultiplierChange}
          />
        </div>

        <div className="mb-8">
          <InfoDisplay
            multiplier={selectedGame.multiplier_value / 100}
          />
        </div>

        {/* Central Bitcoin Betting Section */}
        <div className="mb-8 flex justify-center">
          <Card className="bg-gray-800 border-gray-700 max-w-2xl w-full">
            <CardContent className="p-6">
              <BitcoinAddressSection
                multiplier={selectedGame.multiplier_value / 100}
                targetNumber={selectedGame.max_roll}
                address={selectedGame.address}
              />

              <div className="mt-6 p-4 bg-gray-700 rounded-lg">
                <h3 className="text-lg font-medium mb-2">How It Works</h3>
                <ul className="space-y-2 text-sm text-gray-300">
                  <li>• Roll range: {gameData.info.roll_range}</li>
                  <li>• Win if: {gameData.info.win_condition}</li>
                  <li>• Target: Less than {selectedGame.max_roll.toLocaleString()}</li>
                  <li>• Win probability: {selectedGame.win_probability.toFixed(2)}%</li>
                  <li>• Min bet: {BETTING_CONFIG.MIN_BET_SATS.toLocaleString()} sats • Max bet: {BETTING_CONFIG.MAX_BET_SATS.toLocaleString()} sats</li>
                </ul>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Live Activity Section */}
        <div className="flex justify-center">
          <Card className="bg-gray-800 border-gray-700 max-w-2xl w-full">
            <CardContent className="p-6">
              <ActivityFeed />
            </CardContent>
          </Card>
        </div>

        <footer className="mt-12 text-center text-gray-500 text-sm">
          <Separator className="mb-6 bg-gray-700" />
          <p>
            Provably fair Bitcoin dice • 1.9% house edge • Instant payouts
          </p>
        </footer>
      </div>
    </div>
  );
};

export default Home;
