import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Slider } from "@/components/ui/slider";
import { Copy, Loader2 } from "lucide-react";
import { toast } from "sonner";
import Navbar from "@/components/Navbar";
import { gameService } from "@/services/gameService";
import { useAsync } from "react-use";
import QRCode from "react-qr-code";
import HowItWorks from "@/components/HowItWorks.tsx";
import ActivityFeed from "@/components/ActivityFeed";

// Calculate bet details based on slider value (2-100)
const calculateBetDetails = (number: number) => {
  const winChance = number;
  const multiplier = (100 / winChance) * 0.99; // 1% house edge
  const bitcoinAddress = `1BTC${number.toString().padStart(2, "0")}${Math.random().toString(36).substring(2, 8)}`;

  return {
    number,
    winChance,
    multiplier: Math.round(multiplier * 100) / 100,
    bitcoinAddress,
  };
};

export default function SatoshisNumber() {
  const [betNumber, setBetNumber] = useState([4]);

  // Scroll to top when component mounts
  useEffect(() => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const gameDataState = useAsync(async () => {
    return await gameService.fetchGameAddresses();
  }, []);

  const { loading: isLoading, error, value: gameDataUnsorted } = gameDataState;
  const gameDataAddresses = gameDataUnsorted
    ? gameDataUnsorted.game_addresses.sort(
        (a, b) => a.multiplier_value - b.multiplier_value,
      )
    : [];

  const selectedAddress =
    gameDataAddresses.length > 0 ? gameDataAddresses[betNumber[0]] : undefined;
  const betDetails = calculateBetDetails(betNumber[0]);

  const onBetNumberUpdate = async (betNumber: number[]) => {
    setBetNumber(betNumber);
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success("Address copied to clipboard!");
    } catch (error) {
      toast.error("Failed to copy address");
    }
  };

  // Calculate max amount that can be sent before it becomes a donation
  const calculateMaxSendAmount = (multiplierValue: number) => {
    const maxPayoutSats = parseInt(
      import.meta.env.VITE_MAX_PAYOUT_SATS || "100000",
    );
    // Max payout = input_amount * multiplier / 100
    // So: max_input_amount = max_payout * 100 / multiplier
    return Math.floor((maxPayoutSats * 100) / multiplierValue);
  };

  const maxSendAmount = selectedAddress
    ? calculateMaxSendAmount(selectedAddress.multiplier_value)
    : 0;

  if (isLoading) {
    return (
      <div className="min-h-screen bg-background">
        <Navbar />
        <div className="flex items-center justify-center h-[calc(100vh-4rem)]">
          <div className="text-center space-y-4">
            <Loader2 className="w-12 h-12 animate-spin mx-auto text-primary" />
            <p className="text-muted-foreground">Loading game data...</p>
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="min-h-screen bg-background">
        <Navbar />
        <div className="flex items-center justify-center h-[calc(100vh-4rem)]">
          <Card className="max-w-md">
            <CardContent className="pt-6">
              <div className="text-center space-y-4">
                <p className="text-destructive">
                  {error.message ||
                    "Failed to load game addresses. Please try again later."}
                </p>
                <Button onClick={() => window.location.reload()}>
                  Try Again
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background">
      <Navbar />

      <div className="max-w-6xl mx-auto p-6 space-y-8">
        {/* Game Header */}
        <div className="text-center space-y-4">
          <h1 className="text-4xl font-bold bg-gradient-to-r from-primary to-orange-500 bg-clip-text text-transparent">
            Satoshi's Number
          </h1>
          <p className="text-muted-foreground max-w-2xl mx-auto">
            Send sats to the address below to play against Satoshi. Satoshi will
            think of a number between 1 and 65535. Win if the number is lower
            than your selected threshold - higher risk means bigger rewards!
          </p>
          <Button
            variant="ghost"
            size="sm"
            className="text-sm text-muted-foreground underline"
            onClick={() => {
              document
                .getElementById("how-it-works")
                ?.scrollIntoView({ behavior: "smooth" });
            }}
          >
            How Provably Fair Works
          </Button>
        </div>

        {/* Betting Interface */}
        <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
          <CardContent className="pt-6 pb-6">
            <div className="space-y-6">
              {/* Slider Section */}
              <div className="space-y-4">
                <div className="text-center">
                  <h3 className="text-xl font-semibold mb-2">
                    The number will be lower than{" "}
                    {selectedAddress?.max_roll || betNumber[0]}
                  </h3>
                  <div className="flex justify-center items-center gap-8 text-lg">
                    <div className="text-center">
                      <div className="text-2xl font-bold text-primary">
                        {selectedAddress?.multiplier ||
                          `${betDetails.multiplier}x`}
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Multiplier
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="text-2xl font-bold text-green-500">
                        {selectedAddress
                          ? `${(selectedAddress.win_probability).toFixed(1)}%`
                          : `${betDetails.winChance}%`}
                      </div>
                      <div className="text-sm text-muted-foreground">
                        Win Chance
                      </div>
                    </div>
                  </div>
                </div>

                <div className="px-4">
                  <Slider
                    value={betNumber}
                    onValueChange={onBetNumberUpdate}
                    max={10}
                    min={0}
                    step={1}
                    className="w-full"
                  />
                  <div className="flex justify-between text-xs text-muted-foreground mt-2">
                    <span>
                      {gameDataAddresses
                        ? Math.max(...gameDataAddresses.map((a) => a.max_roll))
                        : 2}{" "}
                      (Low Risk)
                    </span>
                    <span>
                      {gameDataAddresses
                        ? Math.min(...gameDataAddresses.map((a) => a.max_roll))
                        : 99}{" "}
                      (High Risk)
                    </span>
                  </div>
                </div>
              </div>

              {/* QR Code and Address */}
              <div className="flex flex-col items-center gap-4">
                {/* QR Code */}
                <div className="text-center space-y-3">
                  <div className="text-center space-y-2">
                    <QRCode
                      value={`${selectedAddress?.address}`}
                      className="mx-auto border border-border rounded-lg"
                    />
                  </div>

                  {/* Address */}
                  <div className="flex items-center gap-2 justify-center">
                    <p className="font-mono text-sm text-muted-foreground text-center break-all max-w-[200px]">
                      {selectedAddress?.address || betDetails.bitcoinAddress}
                    </p>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() =>
                        copyToClipboard(
                          selectedAddress?.address || betDetails.bitcoinAddress,
                        )
                      }
                      className="h-8 w-8 p-0"
                    >
                      <Copy className="w-4 h-4" />
                    </Button>
                  </div>

                  {/* Instructions */}
                  <div className="text-sm text-muted-foreground max-w-md mx-auto space-y-3">
                    <p>
                      Send Bitcoin to the address above, then roll the dice. If
                      the result is {selectedAddress?.max_roll || betNumber[0]}{" "}
                      or lower, you win{" "}
                      {selectedAddress?.multiplier ||
                        `${betDetails.multiplier}x`}{" "}
                      your bet!
                    </p>

                    {maxSendAmount > 0 && (
                      <div className="bg-card/50 border border-border/50 rounded-lg p-3">
                        <p className="text-xs font-medium text-muted-foreground mb-1">
                          💡 Donation Threshold
                        </p>
                        <p className="text-xs">
                          Amounts over{" "}
                          <span className="font-semibold text-foreground">
                            {maxSendAmount.toLocaleString()} sats
                          </span>{" "}
                          will be treated as donations and won't participate in
                          the game. This helps us manage risk while still
                          accepting generous contributions! 💖
                        </p>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Activity Feed Section */}
        <Card className="bg-gradient-to-br from-card to-card/50 border-primary/20">
          <CardContent className="pt-6">
            <ActivityFeed />
          </CardContent>
        </Card>

        {/* How It Works Section */}
        <section id="how-it-works" className="py-16 border-t border-border/50">
          <HowItWorks />
        </section>
      </div>
    </div>
  );
}
