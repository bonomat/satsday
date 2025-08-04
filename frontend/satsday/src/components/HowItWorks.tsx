import {Card, CardContent, CardHeader, CardTitle} from "@/components/ui/card";
import {Badge} from "@/components/ui/badge";
import {Bitcoin, CheckCircle, Dice6, Hash} from "lucide-react";

export default function HowItWorksContent() {
    const steps = [
        {
            icon: Bitcoin,
            title: "Ark Transaction",
            description: "Your Ark transaction hash is selected as the randomness source.",
            detail: "We use the transaction hash you sent to us to ensure true randomness that cannot be manipulated.",
        },
        {
            icon: Hash,
            title: "Seed Combination",
            description: "The Bitcoin hash is combined with client and server seeds.",
            detail: "Your transaction hash + our server seed = provably fair randomness.",
        },
        {
            icon: Dice6,
            title: "Result Generation",
            description: "The combined hash generates a number from 1-65535 for the dice result.",
            detail: "Mathematical algorithms convert the hash into a fair dice roll you can verify.",
        },
        {
            icon: CheckCircle,
            title: "Verification",
            description: "You can verify the fairness of every single roll independently.",
            detail: "All data is provided so you can recreate and verify the result yourself.",
        },
    ];

    return (
        <div className="max-w-6xl mx-auto p-6 space-y-8">
            <div className="text-center space-y-4">
                <h2 className="text-3xl font-bold">How Provably Fair Works</h2>
                <p className="text-muted-foreground max-w-2xl mx-auto">
                    Our system uses Ark transaction data to generate truly random and verifiable dice results.
                    Here's how we guarantee fairness in every roll.
                </p>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
                {steps.map((step, index) => {
                    const Icon = step.icon;
                    return (
                        <Card key={index}
                              className="relative bg-gradient-to-br from-card to-card/50 border-primary/20 hover:border-primary/40 transition-all duration-300">
                            <CardHeader className="text-center">
                                <div
                                    className="w-16 h-16 bg-primary/10 rounded-full flex items-center justify-center mx-auto mb-4 relative">
                                    <Icon className="w-8 h-8 text-primary"/>
                                    <Badge
                                        className="absolute -top-2 -right-2 w-6 h-6 rounded-full p-0 flex items-center justify-center bg-primary text-primary-foreground">
                                        {index + 1}
                                    </Badge>
                                </div>
                                <CardTitle className="text-lg">{step.title}</CardTitle>
                            </CardHeader>
                            <CardContent className="text-center space-y-3">
                                <p className="text-sm text-muted-foreground">{step.description}</p>
                                <div className="bg-muted/50 p-3 rounded-lg">
                                    <p className="text-xs text-muted-foreground">{step.detail}</p>
                                </div>
                            </CardContent>
                        </Card>
                    );
                })}
            </div>

            {/* Technical Details */}
            <Card className="border-primary/20">
                <CardHeader>
                    <CardTitle className="text-center">Technical Implementation</CardTitle>
                </CardHeader>
                <CardContent className="space-y-6">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div className="space-y-4">
                            <h4 className="font-semibold text-primary">Randomness Source</h4>
                            <ul className="space-y-2 text-sm text-muted-foreground">
                                <li>• Client transaction hash (256-bit entropy)</li>
                                <li>• Server seed (pre-committed hash)</li>
                            </ul>
                        </div>
                        <div className="space-y-4">
                            <h4 className="font-semibold text-primary">Verification Process</h4>
                            <ul className="space-y-2 text-sm text-muted-foreground">
                                <li>• SHA-256 hash combination</li>
                                <li>• Open source verification tools</li>
                                <li>• Complete audit trail</li>
                            </ul>
                        </div>
                    </div>

                    {/* TODO: fix the description */}
                    <div className="bg-muted/30 p-4 rounded-lg">
                        <p className="text-sm text-muted-foreground text-center">
                            <strong>Formula:</strong> HMAC-SHA256(client_tx, server_seed) → Dice
                            Result (1-65535)
                        </p>
                    </div>
                </CardContent>
            </Card>
        </div>
    );
}