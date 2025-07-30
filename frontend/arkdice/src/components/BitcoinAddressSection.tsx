import  { useState, useEffect } from "react";
import { Copy, Check } from "lucide-react";
import { Card, CardContent } from "./ui/card";
import { Button } from "./ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./ui/tooltip";
import { BETTING_CONFIG } from "@/config/betting";

interface BitcoinAddressSectionProps {
  multiplier: number;
  targetNumber: number;
  address?: string;
}

const BitcoinAddressSection = ({
  multiplier = 2.0,
  targetNumber = 5000,
  address = "1dice8EMZmqKvrGE4Qc9bUFf9PX3xaYDp",
}: BitcoinAddressSectionProps) => {
  const [copied, setCopied] = useState(false);
  const [qrCodeUrl, setQrCodeUrl] = useState("");

  useEffect(() => {
    // Generate QR code URL based on the Bitcoin address
    // Using a public QR code generator service
    setQrCodeUrl(
      `https://api.qrserver.com/v1/create-qr-code/?size=200x200&data=${address}`,
    );
  }, [address]);

  const handleCopyAddress = () => {
    navigator.clipboard.writeText(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="w-full max-w-md mx-auto bg-background p-6 rounded-xl border border-border">
      <h2 className="text-xl font-bold mb-4 text-center">
        Send Sats to Win
      </h2>

      <Card className="mb-6">
        <CardContent className="p-6 flex flex-col items-center">
          {/* QR Code */}
          <div className="mb-4 p-2 bg-white rounded-lg">
            <img
              src={qrCodeUrl}
              alt="Bitcoin Address QR Code"
              className="w-48 h-48"
            />
          </div>

          {/* Target Number Indicator */}
          <div className="mb-4 text-center">
            <span className="text-sm text-muted-foreground">
              Target Number:
            </span>
            <div className="text-xl font-mono font-bold">
              Less than{" "}
              <span className="text-orange-500">
                {targetNumber.toLocaleString()}
              </span>
            </div>
          </div>

          {/* Bitcoin Address */}
          <div className="w-full bg-muted p-3 rounded-md mb-3">
            <div className="font-mono text-sm break-all text-center">
              {address}
            </div>
          </div>

          {/* Copy Button */}
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  onClick={handleCopyAddress}
                  variant="outline"
                  className="w-full flex items-center justify-center gap-2"
                >
                  {copied ? (
                    <>
                      <Check className="h-4 w-4" />
                      Copied!
                    </>
                  ) : (
                    <>
                      <Copy className="h-4 w-4" />
                      Copy Address
                    </>
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                <p>Copy Bitcoin address to clipboard</p>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </CardContent>
      </Card>

      {/* Multiplier Label */}
      <div className="text-center text-sm text-muted-foreground">
        <span className="font-semibold">{address.substring(0, 8)}...</span> -
        <span className="text-orange-500 font-bold">
          {multiplier.toFixed(2)}x
        </span>{" "}
        multiplier
      </div>

      {/* How It Works */}
      <div className="mt-4 text-sm text-muted-foreground">
        <p className="mb-2">
          Send sats to the address above. If the lucky number is less than{" "}
          {targetNumber.toLocaleString()}, you win {multiplier.toFixed(2)}x your bet!
        </p>
        <ul className="list-disc list-inside space-y-1">
          <li>Minimum bet: {BETTING_CONFIG.MIN_BET_SATS.toLocaleString()} sats</li>
          <li>Maximum bet: {BETTING_CONFIG.MAX_BET_SATS.toLocaleString()} sats</li>
          <li>House edge: 1.9%</li>
          <li>1 confirmation required</li>
        </ul>
      </div>
    </div>
  );
};

export default BitcoinAddressSection;
