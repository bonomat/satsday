import { useState, useEffect, useCallback, useRef } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Slider } from "@/components/ui/slider";
import { Copy, Loader2, Shield, Dices } from "lucide-react";
import { toast } from "sonner";
import { Link } from "react-router-dom";
import Navbar from "@/components/Navbar";
import Footer from "@/components/Footer";
import { gameService } from "@/services/gameService";
import { useAsync } from "react-use";
import QRCode from "react-qr-code";
import HowItWorks from "@/components/HowItWorks.tsx";
import ActivityFeed from "@/components/ActivityFeed";
import { useWalletBridge } from "@/hooks/useWalletBridge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

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
  const [isDialogOpen, setIsDialogOpen] = useState(false);
  const [amount, setAmount] = useState("1000");
  const [isSending, setIsSending] = useState(false);
  const seenNotifications = useRef(new Set<string>());

  const handlePaymentReceived = useCallback((notification: { address?: string; amount: number; txid: string; timestamp: number; createdAt: number }) => {
    console.log("[SatoshisNumber] Payment received:", notification);

    // Check if we've seen this notification before
    const notificationKey = `${notification.address}-${notification.txid}-${notification.createdAt}`;
    if (seenNotifications.current.has(notificationKey)) {
      console.log("[SatoshisNumber] Notification already seen, skipping:", notificationKey);
      return;
    }

    // Mark as seen
    seenNotifications.current.add(notificationKey);

    // Show celebration toast
    toast.success(
      `Payment received! ${notification.amount.toLocaleString()} sats`,
      {
        description: `TXID: ${notification.txid.substring(0, 12)}...`,
        duration: 5000,
      }
    );

    // You can add more animations here (e.g., confetti)
  }, []);

  const { isAvailable: isBridgeAvailable, isChecking: isBridgeChecking, client: bridgeClient } = useWalletBridge(handlePaymentReceived);

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
      console.log("[SatoshisNumber] Address copied to clipboard:", text);
      toast.success("Address copied to clipboard!");
    } catch (error) {
      console.error("[SatoshisNumber] Failed to copy address:", error);
      toast.error("Failed to copy address");
    }
  };

  const handleRollTheDice = async () => {
    if (!bridgeClient || !selectedAddress) {
      console.error("[SatoshisNumber] Bridge client not available");
      toast.error("Bridge client not available");
      return;
    }

    const amountSats = parseInt(amount);
    if (isNaN(amountSats) || amountSats <= 0) {
      console.error("[SatoshisNumber] Invalid amount:", amount);
      toast.error("Please enter a valid amount");
      return;
    }

    if (maxSendAmount > 0 && amountSats > maxSendAmount) {
      console.error("[SatoshisNumber] Amount exceeds maximum:", amountSats, "max:", maxSendAmount);
      toast.error(`Maximum bet amount is ${maxSendAmount.toLocaleString()} sats`);
      return;
    }

    setIsSending(true);
    try {
      console.log("[SatoshisNumber] Sending to address:", selectedAddress.address, "amount:", amountSats);
      const txid = await bridgeClient.sendToAddress(
        selectedAddress.address,
        amountSats
      );
      console.log("[SatoshisNumber] Transaction sent! TXID:", txid);
      toast.success(`Transaction sent! TXID: ${txid.substring(0, 8)}...`);
      setIsDialogOpen(false);
      setAmount("1000"); // Reset to default
    } catch (error) {
      console.error("[SatoshisNumber] Failed to send:", error);
      toast.error(`Failed to send: ${error instanceof Error ? error.message : "Unknown error"}`);
    } finally {
      setIsSending(false);
    }
  };

  const maxSendAmount = selectedAddress ? selectedAddress.max_bet_amount : 0;

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
          <div className="flex flex-col sm:flex-row gap-2 justify-center">
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
            <Link to="/verify">
              <Button
                variant="ghost"
                size="sm"
                className="text-sm text-muted-foreground underline"
              >
                <Shield className="w-4 h-4 mr-1" />
                Verify Game Result
              </Button>
            </Link>
          </div>
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

                  {/* Roll the Dice Button - Only shown when bridge is available */}
                  {!isBridgeChecking && isBridgeAvailable && (
                    <div className="mt-4">
                      <Button
                        onClick={() => setIsDialogOpen(true)}
                        className="w-full max-w-xs bg-gradient-to-r from-primary to-orange-500 hover:from-primary/90 hover:to-orange-500/90"
                        size="lg"
                      >
                        <Dices className="w-5 h-5 mr-2" />
                        Roll the Dice
                      </Button>
                    </div>
                  )}

                  {/* Instructions */}
                  <div className="text-sm text-muted-foreground max-w-md mx-auto space-y-3">
                    <p>
                      Send Bitcoin to the address above, which will roll the dice. If
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

      <Footer />

      {/* Roll the Dice Dialog */}
      <Dialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <DialogTitle>Roll the Dice</DialogTitle>
            <DialogDescription>
              Enter the amount you want to bet. If Satoshi's number is{" "}
              {selectedAddress?.max_roll || betNumber[0]} or lower, you win{" "}
              {selectedAddress?.multiplier || `${betDetails.multiplier}x`} your bet!
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label htmlFor="amount" className="text-right">
                Amount
              </Label>
              <div className="col-span-3 relative">
                <Input
                  id="amount"
                  type="number"
                  value={amount}
                  onChange={(e) => setAmount(e.target.value)}
                  placeholder="1000"
                  className="pr-12"
                  disabled={isSending}
                />
                <span className="absolute right-3 top-1/2 -translate-y-1/2 text-sm text-muted-foreground">
                  sats
                </span>
              </div>
            </div>
            {maxSendAmount > 0 && (
              <div className="text-xs text-muted-foreground">
                Maximum bet: {maxSendAmount.toLocaleString()} sats
              </div>
            )}
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setIsDialogOpen(false)}
              disabled={isSending}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              onClick={handleRollTheDice}
              disabled={isSending}
              className="bg-gradient-to-r from-primary to-orange-500"
            >
              {isSending ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Sending...
                </>
              ) : (
                <>
                  <Dices className="w-4 h-4 mr-2" />
                  Send & Roll
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
