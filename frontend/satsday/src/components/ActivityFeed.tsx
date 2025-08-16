import { ScrollArea } from "./ui/scroll-area";
import { Badge } from "./ui/badge";
import { Separator } from "./ui/separator";
import {
  Clock,
  ArrowUpRight,
  ArrowDownRight,
  ExternalLink,
  Copy,
  CheckCircle,
  Heart,
} from "lucide-react";
import { useState, useMemo } from "react";
import { useGameWebSocket, DonationItem } from "@/hooks/useGameWebSocket";
import { Button } from "./ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "./ui/tooltip";
import { formatDistanceToNow } from "date-fns";

type FeedItem = {
  type: "game" | "donation";
  timestamp: number;
  data: any;
};

const ActivityFeed = () => {
  const {
    activities,
    donations,
    isConnected: wsConnected,
    isLoading: loading,
  } = useGameWebSocket(20);
  const [copiedTxId, setCopiedTxId] = useState<string | null>(null);

  // Combine and sort activities and donations by timestamp
  const combinedFeed = useMemo(() => {
    const gameItems: FeedItem[] = activities.map((activity) => ({
      type: "game" as const,
      timestamp: activity.timestamp,
      data: activity,
    }));

    const donationItems: FeedItem[] = donations.map((donation) => ({
      type: "donation" as const,
      timestamp: donation.timestamp,
      data: donation,
    }));

    return [...gameItems, ...donationItems]
      .sort((a, b) => b.timestamp - a.timestamp)
      .slice(0, 20);
  }, [activities, donations]);

  const copyToClipboard = async (text: string, txType: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedTxId(`${txType}-${text}`);
      setTimeout(() => setCopiedTxId(null), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const truncateTxId = (txId: string) => {
    return `${txId.slice(0, 6)}...${txId.slice(-6)}`;
  };

  const renderDonationItem = (donation: DonationItem) => {
    const timeAgo = formatDistanceToNow(donation.timestamp * 1000);
    return (
      <div key={donation.id} className="space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Clock className="h-4 w-4 text-gray-400" />
            <span className="text-sm text-gray-400">{timeAgo} ago</span>
          </div>
          <Badge className="bg-pink-600 hover:bg-pink-700">
            <Heart className="h-3 w-3 mr-1" />
            DONATION
          </Badge>
        </div>

        <div className="flex justify-between items-center">
          <div>
            <p className="text-sm font-medium text-gray-300">
              Amount: <span className="text-white">{donation.amount} sats</span>
            </p>
            <p className="text-sm font-medium text-gray-300">
              From:{" "}
              <span className="text-white font-mono text-xs">
                {donation.sender.length > 20
                  ? `${donation.sender.slice(0, 10)}...${donation.sender.slice(-6)}`
                  : donation.sender}
              </span>
            </p>
          </div>
          <div className="text-right">
            <Heart className="h-8 w-8 text-pink-500 mx-auto mb-1" />
            <p className="text-xs text-pink-400">Thank you!</p>
          </div>
        </div>

        <div className="space-y-1">
          <div className="flex items-center gap-2 text-xs">
            <span className="text-gray-400">Donation TX:</span>
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-auto p-0 text-pink-400 hover:text-pink-300"
                    onClick={() =>
                      copyToClipboard(donation.input_tx_id, "donation")
                    }
                  >
                    <code>{truncateTxId(donation.input_tx_id)}</code>
                    {copiedTxId === `donation-${donation.input_tx_id}` ? (
                      <CheckCircle className="h-3 w-3 ml-1" />
                    ) : (
                      <Copy className="h-3 w-3 ml-1" />
                    )}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>
                  <p>Click to copy: {donation.input_tx_id}</p>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
        </div>

        <Separator className="bg-gray-700 mt-2" />
      </div>
    );
  };

  if (loading) {
    return (
      <div className="w-full">
        <h2 className="text-xl font-bold text-orange-500 mb-4">
          Recent Activity
        </h2>
        <div className="flex items-center justify-center h-[400px]">
          <p className="text-gray-400">Loading games...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold text-orange-500">Recent Activity</h2>
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${wsConnected ? "bg-green-500" : "bg-red-500"}`}
          />
          <span className="text-xs text-gray-400">
            {wsConnected ? "Live" : "Reconnecting..."}
          </span>
        </div>
      </div>
      <ScrollArea className="h-[400px]">
        <div className="space-y-4 pr-4">
          {combinedFeed.length === 0 ? (
            <div className="flex items-center justify-center h-[350px]">
              <p className="text-gray-400">No activity yet</p>
            </div>
          ) : (
            combinedFeed.map((item) => {
              if (item.type === "donation") {
                return renderDonationItem(item.data);
              }

              // Game item rendering
              const activity = item.data;
              const timeAgo = formatDistanceToNow(activity.timestamp * 1000);
              return (
                <div key={activity.id} className="space-y-2">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-2">
                      <Clock className="h-4 w-4 text-gray-400" />
                      <span className="text-sm text-gray-400">
                        {timeAgo} ago
                      </span>
                    </div>
                    <Badge
                      variant={activity.is_win ? "default" : "destructive"}
                      className={
                        activity.is_win ? "bg-green-600 hover:bg-green-700" : ""
                      }
                    >
                      {activity.is_win ? "WIN" : "LOSS"}
                    </Badge>
                  </div>

                  <div className="flex justify-between items-center">
                    <div>
                      <p className="text-sm font-medium text-gray-300">
                        Sent:{" "}
                        <span className="text-white">
                          {activity.amount_sent} sats
                        </span>
                      </p>
                      <p className="text-sm font-medium text-gray-300">
                        Multiplier:{" "}
                        <span className="text-white">
                          {activity.multiplier}x
                        </span>
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-sm font-medium text-gray-300">
                        Result:{" "}
                        <span className="text-white">
                          {activity.result_number}
                        </span>
                      </p>
                      <p className="text-sm font-medium text-gray-300">
                        Target:{" "}
                        <span className="text-white">
                          &lt; {activity.target_number}
                        </span>
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-1">
                      {activity.is_win ? (
                        <ArrowUpRight className="h-4 w-4 text-green-500" />
                      ) : (
                        <ArrowDownRight className="h-4 w-4 text-red-500" />
                      )}
                      <span
                        className={`text-sm font-medium ${activity.is_win ? "text-green-500" : "text-red-500"}`}
                      >
                        Payout:{" "}
                        {activity.payout ? `${activity.payout} sats` : "None"}
                      </span>
                    </div>
                  </div>

                  <div className="space-y-1">
                    <div className="flex items-center gap-2 text-xs">
                      <span className="text-gray-400">Game TX:</span>
                      <TooltipProvider>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-auto p-0 text-orange-400 hover:text-orange-300"
                              onClick={() =>
                                copyToClipboard(activity.input_tx_id, "input")
                              }
                            >
                              <code>{truncateTxId(activity.input_tx_id)}</code>
                              {copiedTxId ===
                              `input-${activity.input_tx_id}` ? (
                                <CheckCircle className="h-3 w-3 ml-1" />
                              ) : (
                                <Copy className="h-3 w-3 ml-1" />
                              )}
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            <p>Click to copy: {activity.input_tx_id}</p>
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                      {/*TODO: add a link to verify this game */}
                      {/*<a*/}
                      {/*    href={`https://mempool.space/tx/${activity.input_tx_id}`}*/}
                      {/*    target="_blank"*/}
                      {/*    rel="noopener noreferrer"*/}
                      {/*    className="text-orange-400 hover:text-orange-300"*/}
                      {/*>*/}
                      {/*    <ExternalLink className="h-3 w-3"/>*/}
                      {/*</a>*/}
                    </div>

                    {activity.output_tx_id && (
                      <div className="flex items-center gap-2 text-xs">
                        <span className="text-gray-400">Payout TX:</span>
                        <TooltipProvider>
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <Button
                                variant="ghost"
                                size="sm"
                                className="h-auto p-0 text-orange-400 hover:text-orange-300"
                                onClick={() =>
                                  copyToClipboard(
                                    activity.output_tx_id!,
                                    "output",
                                  )
                                }
                              >
                                <code>
                                  {truncateTxId(activity.output_tx_id)}
                                </code>
                                {copiedTxId ===
                                `output-${activity.output_tx_id}` ? (
                                  <CheckCircle className="h-3 w-3 ml-1" />
                                ) : (
                                  <Copy className="h-3 w-3 ml-1" />
                                )}
                              </Button>
                            </TooltipTrigger>
                            <TooltipContent>
                              <p>Click to copy: {activity.output_tx_id}</p>
                            </TooltipContent>
                          </Tooltip>
                        </TooltipProvider>
                        {/*<a*/}
                        {/*    href={`https://mempool.space/tx/${activity.output_tx_id}`}*/}
                        {/*    target="_blank"*/}
                        {/*    rel="noopener noreferrer"*/}
                        {/*    className="text-orange-400 hover:text-orange-300"*/}
                        {/*>*/}
                        {/*    <ExternalLink className="h-3 w-3"/>*/}
                        {/*</a>*/}
                      </div>
                    )}

                    {activity.nonce ? (
                      <>
                        <div className="flex items-center gap-2 text-xs">
                          <span className="text-gray-400">Nonce:</span>
                          <TooltipProvider>
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <code className="text-gray-300">
                                  {activity.nonce}
                                </code>
                              </TooltipTrigger>
                              <TooltipContent>
                                <p>Used for provably fair verification</p>
                              </TooltipContent>
                            </Tooltip>
                          </TooltipProvider>
                        </div>
                      </>
                    ) : (
                      <>
                        <div className="flex items-center gap-2 text-xs">
                          <span className="text-gray-400">Nonce Hash:</span>
                          <TooltipProvider>
                            <Tooltip>
                              <TooltipTrigger asChild>
                                <code className="text-gray-300">
                                  {activity.nonce_hash}
                                </code>
                              </TooltipTrigger>
                              <TooltipContent>
                                <p>Used for provably fair verification</p>
                              </TooltipContent>
                            </Tooltip>
                          </TooltipProvider>
                        </div>
                      </>
                    )}
                  </div>

                  <Separator className="bg-gray-700 mt-2" />
                </div>
              );
            })
          )}
        </div>
      </ScrollArea>
    </div>
  );
};

export default ActivityFeed;
