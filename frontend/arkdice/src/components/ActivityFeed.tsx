import React from "react";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { ScrollArea } from "./ui/scroll-area";
import { Badge } from "./ui/badge";
import { Separator } from "./ui/separator";
import { Clock, ArrowUpRight, ArrowDownRight } from "lucide-react";

interface ActivityItem {
  id: string;
  timeAgo: string;
  amountSent: string;
  multiplier: number;
  resultNumber: number;
  targetNumber: number;
  isWin: boolean;
  payout: string;
}

interface ActivityFeedProps {
  activities?: ActivityItem[];
}

const ActivityFeed = ({ activities = [] }: ActivityFeedProps) => {
  // Default mock data if no activities are provided
  const defaultActivities: ActivityItem[] = [
    {
      id: "1",
      timeAgo: "2 min ago",
      amountSent: "0.015 BTC",
      multiplier: 2.5,
      resultNumber: 32,
      targetNumber: 40,
      isWin: true,
      payout: "0.0375 BTC",
    },
    {
      id: "2",
      timeAgo: "5 min ago",
      amountSent: "0.025 BTC",
      multiplier: 5.0,
      resultNumber: 65,
      targetNumber: 20,
      isWin: false,
      payout: "0 BTC",
    },
    {
      id: "3",
      timeAgo: "12 min ago",
      amountSent: "0.005 BTC",
      multiplier: 10.0,
      resultNumber: 8,
      targetNumber: 10,
      isWin: true,
      payout: "0.05 BTC",
    },
    {
      id: "4",
      timeAgo: "18 min ago",
      amountSent: "0.01 BTC",
      multiplier: 3.0,
      resultNumber: 45,
      targetNumber: 33,
      isWin: false,
      payout: "0 BTC",
    },
    {
      id: "5",
      timeAgo: "25 min ago",
      amountSent: "0.008 BTC",
      multiplier: 1.5,
      resultNumber: 42,
      targetNumber: 66,
      isWin: true,
      payout: "0.012 BTC",
    },
    {
      id: "6",
      timeAgo: "32 min ago",
      amountSent: "0.02 BTC",
      multiplier: 4.0,
      resultNumber: 15,
      targetNumber: 25,
      isWin: true,
      payout: "0.08 BTC",
    },
    {
      id: "7",
      timeAgo: "45 min ago",
      amountSent: "0.03 BTC",
      multiplier: 2.0,
      resultNumber: 75,
      targetNumber: 50,
      isWin: false,
      payout: "0 BTC",
    },
  ];

  const displayActivities =
    activities.length > 0 ? activities : defaultActivities;

  return (
    <Card className="w-full max-w-md bg-black border-zinc-800">
      <CardHeader className="pb-2">
        <CardTitle className="text-xl font-bold text-orange-500">
          Recent Activity
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">
        <ScrollArea className="h-[400px] pr-4">
          <div className="space-y-4 p-4">
            {displayActivities.map((activity) => (
              <div key={activity.id} className="space-y-2">
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    <Clock className="h-4 w-4 text-zinc-400" />
                    <span className="text-sm text-zinc-400">
                      {activity.timeAgo}
                    </span>
                  </div>
                  <Badge
                    variant={activity.isWin ? "default" : "destructive"}
                    className={
                      activity.isWin ? "bg-green-600 hover:bg-green-700" : ""
                    }
                  >
                    {activity.isWin ? "WIN" : "LOSS"}
                  </Badge>
                </div>

                <div className="flex justify-between items-center">
                  <div>
                    <p className="text-sm font-medium text-zinc-300">
                      Sent:{" "}
                      <span className="text-white">{activity.amountSent}</span>
                    </p>
                    <p className="text-sm font-medium text-zinc-300">
                      Multiplier:{" "}
                      <span className="text-white">{activity.multiplier}x</span>
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-medium text-zinc-300">
                      Result:{" "}
                      <span className="text-white">
                        {activity.resultNumber}
                      </span>
                    </p>
                    <p className="text-sm font-medium text-zinc-300">
                      Target:{" "}
                      <span className="text-white">
                        &lt; {activity.targetNumber}
                      </span>
                    </p>
                  </div>
                </div>

                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-1">
                    {activity.isWin ? (
                      <ArrowUpRight className="h-4 w-4 text-green-500" />
                    ) : (
                      <ArrowDownRight className="h-4 w-4 text-red-500" />
                    )}
                    <span
                      className={`text-sm font-medium ${activity.isWin ? "text-green-500" : "text-red-500"}`}
                    >
                      Payout: {activity.payout}
                    </span>
                  </div>
                </div>

                <Separator className="bg-zinc-800 mt-2" />
              </div>
            ))}
          </div>
        </ScrollArea>
      </CardContent>
    </Card>
  );
};

export default ActivityFeed;
